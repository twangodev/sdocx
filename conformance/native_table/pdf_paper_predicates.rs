use super::*;

const WDOC: u64 = 0x0800_0000;
const WDOC_SHA256: &str = "1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6";
const PAGE: u64 = MODEL + 0x10000;
const IMPLEMENTATION: u64 = MODEL + 0x11000;
const LIST: u64 = IMPLEMENTATION + 0x178;
const RECORDS: u64 = MODEL + 0x12000;
const RECORD_BYTES: u64 = 0x48;
const HAS_PDF: u64 = WDOC + 0xc6228;
const HAS_PDF_BINDING: u64 = WDOC + 0xc628c;
const GET_PDF_DATA: u64 = WDOC + 0xc61d0;
const QUERIES: [i32; 7] = [-1, 0, 7, 42, 99, i32::MIN, i32::MAX];
const FILLS: [u8; 5] = [0, 0x55, 0xa5, 0xff, 0];

#[derive(Clone, Copy)]
struct Record {
    binding: i32,
    page_index: i32,
}

struct Case {
    name: &'static str,
    records: &'static [Record],
}

impl Case {
    fn execute(&self, machine: &Machine, fill: u8) -> String {
        write(machine.engine, PAGE, &[0; 32]);
        write(machine.engine, IMPLEMENTATION, &[0; 512]);
        write(machine.engine, PAGE + 16, &IMPLEMENTATION.to_le_bytes());
        let constructed = machine.call(frames::BASE + 0xd133c, &[LIST]);
        assert_eq!(constructed, 1, "native ArrayList construction failed");
        let mut additions = Vec::new();
        for (index, record) in self.records.iter().enumerate() {
            let pointer = RECORDS + index as u64 * RECORD_BYTES;
            write(machine.engine, pointer, &[fill; RECORD_BYTES as usize]);
            write(machine.engine, pointer + 16, &record.binding.to_le_bytes());
            write(
                machine.engine,
                pointer + 20,
                &record.page_index.to_le_bytes(),
            );
            let accepted = machine.call(frames::BASE + 0xd15b0, &[LIST, pointer]);
            assert_eq!(accepted, 1, "native ArrayList insertion failed");
            additions.push(accepted);
        }
        let returned_list = machine.call(GET_PDF_DATA, &[PAGE]);
        assert_eq!(returned_list, LIST, "native getter changed list identity");
        let count = machine.call(frames::BASE + 0xd17f4, &[returned_list]);
        assert_eq!(count as usize, self.records.len());
        let records: Vec<_> = (0..count)
            .map(|index| {
                let pointer = machine.call(frames::BASE + 0xd1c74, &[returned_list, index]);
                format!(
                    "{{\"binding_id\":{},\"page_index\":{}}}",
                    read_u32(machine.engine, pointer + 16) as i32,
                    read_u32(machine.engine, pointer + 20) as i32,
                )
            })
            .collect();
        let has_pdf = machine.call(HAS_PDF, &[PAGE]);
        let queries: Vec<_> = QUERIES
            .into_iter()
            .map(|query| {
                let result = machine.call(HAS_PDF_BINDING, &[PAGE, query as i64 as u64]);
                format!("{{\"binding_id\":{query},\"has_pdf\":{result}}}")
            })
            .collect();
        format!(
            "{{\"name\":{:?},\"array_list_constructed\":{constructed},\"array_list_add_results\":{additions:?},\"get_pdf_data_offset\":{},\"record_count\":{count},\"records\":[{}],\"has_pdf\":{has_pdf},\"queries\":[{}]}}",
            self.name,
            returned_list - IMPLEMENTATION,
            records.join(","),
            queries.join(","),
        )
    }
}

pub(super) fn capture(machine: &mut Machine, base: &Path, wdoc: &Path) {
    map_library(machine.engine, base, frames::BASE, frames::BASE_SHA256);
    map_library(machine.engine, wdoc, WDOC, WDOC_SHA256);
    for (plt, target) in [
        (WDOC + 0xfafa0, frames::BASE + 0xd17f4),
        (WDOC + 0xfafb0, frames::BASE + 0xd1c74),
        (frames::BASE + 0xe56f0, NEW),
        (frames::BASE + 0xe5780, NEW),
        (frames::BASE + 0xe55d0, DELETE),
    ] {
        bind_native(machine.engine, plt, target);
    }
    const FIRST: Record = Record {
        binding: 7,
        page_index: 99,
    };
    const SECOND: Record = Record {
        binding: 42,
        page_index: 7,
    };
    let cases = [
        Case {
            name: "empty-list",
            records: &[],
        },
        Case {
            name: "negative-binding",
            records: &[Record {
                binding: -1,
                page_index: 0,
            }],
        },
        Case {
            name: "zero-page-index",
            records: &[Record {
                binding: 7,
                page_index: 0,
            }],
        },
        Case {
            name: "same-binding-different-page-index",
            records: &[FIRST],
        },
        Case {
            name: "two-bindings",
            records: &[FIRST, SECOND],
        },
        Case {
            name: "duplicate-bindings-and-negative-page-index",
            records: &[
                FIRST,
                Record {
                    binding: 7,
                    page_index: -1,
                },
                SECOND,
            ],
        },
        Case {
            name: "signed-binding-extremes",
            records: &[
                Record {
                    binding: i32::MIN,
                    page_index: i32::MAX,
                },
                Record {
                    binding: i32::MAX,
                    page_index: i32::MIN,
                },
            ],
        },
    ];
    let mut canonical = None;
    for fill in FILLS {
        machine.heap.cursor = HEAP;
        machine.heap.allocation_fill = fill;
        machine.heap.allocations = 0;
        machine.heap.fills = 0;
        machine.heap.deletes = 0;
        let output = cases
            .iter()
            .map(|case| case.execute(machine, fill))
            .collect::<Vec<_>>()
            .join(",\n");
        if let Some(expected) = &canonical {
            assert_eq!(&output, expected, "native output changed for fill {fill}");
        } else {
            canonical = Some(output);
        }
    }
    println!(
        concat!(
            "{{\"apk_version\":\"4.4.45.37\",",
            "\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",",
            "\"model_library_sha256\":\"{}\",\"base_library_sha256\":\"{}\",\"wdoc_library_sha256\":\"{}\",",
            "\"allocation_fills\":{:?},\"native_addresses\":{{\"get_pdf_data\":\"0xc61d0\",\"has_pdf\":\"0xc6228\",\"has_pdf_binding\":\"0xc628c\",\"array_list_construct\":\"Base:0xd133c\",\"array_list_add\":\"Base:0xd15b0\",\"array_list_count\":\"Base:0xd17f4\",\"array_list_get\":\"Base:0xd1c74\"}},",
            "\"capture_boundary\":\"Complete unchanged public WPage GetPDFData/HasPDF()/HasPDF(int) and actual Base ArrayList construction/insertion/count/indexing execute. Caller supplies zeroed WPage facade/implementation with facade+16 pointing to implementation and the list at implementation+0x178, plus 72-byte record storage with explicit binding ID at +16 and PDF page index at +20. Unread record bytes and owned native allocations vary across five fills; repeated zero fill agrees. Host bounded allocation/free only; other called imports are not substituted. No WPage/PDFData construction, list setters/deep copy, serialization, ModelContext/media availability, document loading, Composer/PDF placement, rendering or pixels execute. Negative query IDs describe predicate behavior on supplied records, not valid document/resource binding policy.\",",
            "\"cases\":[\n{}\n]}}"
        ),
        LIBRARY_SHA256,
        frames::BASE_SHA256,
        WDOC_SHA256,
        FILLS,
        canonical.unwrap(),
    );
}
