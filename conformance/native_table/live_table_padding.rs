use super::*;

fn capacities(machine: &Machine) -> String {
    let cells = (0..4).map(|slot| {
        let cell = machine.call(0x3d2be0, &[TABLE, slot / 2, slot % 2]);
        let child = machine.call(CELL_DRAWING + 0xacb84, &[LAYOUT, cell]);
        let wrapper = widget_text_constructor::text_wrapper(machine, child);
        let layout = read_u64(machine.engine, wrapper + 64) + 192;
        let stored = read_u32(machine.engine, layout + 276);
        let getter = float_getter(machine, TEXT + 0x70f64, layout);
        assert_eq!(stored, getter);
        let begin = read_u64(machine.engine, layout + 184);
        let end = read_u64(machine.engine, layout + 192);
        assert!(end >= begin && (end - begin).is_multiple_of(16) && end - begin <= 128);
        let rectangles = (begin..end).step_by(16).map(|pointer| {
            std::array::from_fn::<_,4,_>(|axis|read_u32(machine.engine,pointer+axis as u64*4))
        }).collect::<Vec<_>>();
        format!("{{\"slot\":{slot},\"stored_max_character_height_bits\":{stored},\"native_getter_max_character_height_bits\":{getter},\"sorted_native_padding_rect_bits\":{rectangles:?}}}")
    }).collect::<Vec<_>>();
    format!("[{}]", cells.join(","))
}

pub(super) fn capture(machine: &mut Machine, paths: widget_text_constructor::Paths<'_>) {
    let (mut environment, fonts) = setup_with_preloaded_libraries(
        machine,
        paths.base,
        paths.text,
        paths.skia,
        paths.font,
        paths.xml,
        paths.cpp,
        &[
            (paths.widget, WIDGET, geometry::WIDGET_SHA256),
            (paths.content, CONTENT, CONTENT_SHA256),
            (paths.drawing, CELL_DRAWING, DRAWING_SHA256),
        ],
        &[(paths.model, 0, LIBRARY_SHA256)],
    );
    let mut host = Box::new(Host {
        cell: install_host(machine, &mut environment, fonts),
        math_calls: BTreeMap::new(),
    });
    environment.set_host_import_handler(host_import, ptr::from_mut(host.as_mut()).cast());
    CodeObserver::with(machine, Trace::default(), |machine, observation| {
        observation.observe(trace, ENTRIES.map(|(_, address)| address));
        let base = Case {
            name: "no-padding",
            bounds: [13.25, -19.5, 173.25, 180.5],
            texts: ["AV abc", "To", "A😀B", "last"],
            merge: None,
            font_size: None,
            margins: None,
            scale: 1.,
            density: 1.,
            direction: 0,
            bands: &[],
            resize: None,
        };
        let sorted: &[[f32; 4]] = &[
            [0., 20., 180., 40.],
            [0., 120., 180., 150.],
            [0., 155., 180., 180.],
        ];
        let cases = [
            base,
            Case {
                name: "single-full-width-band",
                bands: &[[0., 20., 180., 120.]],
                ..base
            },
            Case {
                name: "first-line-collision-with-height-fallback",
                bands: &[[0., 20., 180., 120.], [0., 140., 180., 250.]],
                ..base
            },
            Case {
                name: "first-pair-capacity-later-smaller-gap",
                bands: sorted,
                ..base
            },
            Case {
                name: "unsorted-first-pair-capacity",
                bands: &[
                    [0., 155., 180., 180.],
                    [0., 20., 180., 40.],
                    [0., 120., 180., 150.],
                ],
                ..base
            },
            Case {
                name: "overlapping-bands-negative-capacity",
                bands: &[[0., 20., 180., 120.], [0., 100., 180., 140.]],
                ..base
            },
            Case {
                name: "touching-bands-zero-capacity",
                bands: &[[0., 20., 180., 120.], [0., 120., 180., 250.]],
                ..base
            },
            Case {
                name: "empty-width-second-band",
                bands: &[[0., 20., 180., 120.], [0., 140., 0., 250.]],
                ..base
            },
            Case {
                name: "font17-capacity12",
                font_size: Some(17.),
                bands: &[[0., 20., 180., 120.], [0., 132., 180., 250.]],
                ..base
            },
            Case {
                name: "asymmetric-margins-capacity20",
                margins: Some([1.25, 2.5, 3.75, 4.5]),
                bands: &[[0., 20., 180., 120.], [0., 140., 180., 250.]],
                ..base
            },
        ];
        let output = cases
            .into_iter()
            .map(|case| {
                let mut canonical = None;
                for fill in [0, 85, 165, 255, 0] {
                    let mut sample =
                        case.sample(machine, &mut environment, &mut host, observation, fill);
                    let capacity = capacities(machine);
                    assert_eq!(sample.pop(), Some('}'));
                    sample.push_str(&format!(",\"after_warm_native_padding\":{capacity}}}"));
                    if let Some(expected) = &canonical {
                        assert!(
                            &sample == expected,
                            "case {} fill {fill} disagrees",
                            case.name
                        );
                    } else {
                        canonical = Some(sample)
                    }
                }
                canonical.unwrap()
            })
            .collect::<Vec<_>>();
        println!(
            "{{\"apk_version\":\"4.4.45.37\",\"apk_sha256\":\"daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667\",\"dependencies\":{},\"memory_fills\":[0,85,165,255],\"repeat_zero_fill\":true,\"model_library_sha256\":{},\"base_library_sha256\":{},\"text_library_sha256\":{},\"skia_library_sha256\":{},\"widget_library_sha256\":{},\"content_library_sha256\":{},\"drawing_library_sha256\":{},\"native_entrypoints\":{{\"table_constructor\":\"Drawing:0xa9d58\",\"cold_measure\":\"Drawing:0xaa530\",\"warm_layout\":\"Drawing:0xaa3d4\",\"set_object_split_offsets\":\"Drawing:0xad040\",\"set_padding_rectangles\":\"Text:0x70d24\",\"max_character_height_getter\":\"Text:0x70f64\",\"line_height_with_spacing\":\"Text:0x8e100\",\"paragraph_baseline\":\"Text:0x6cb0c\"}},\"capture_boundary\":{},\"cases\":[{}]}}",
            dependency_metadata(),
            json_string(LIBRARY_SHA256),
            json_string(frames::BASE_SHA256),
            json_string(geometry::TEXT_SHA256),
            json_string(text_font_source::SKIA_SHA256),
            json_string(geometry::WIDGET_SHA256),
            json_string(CONTENT_SHA256),
            json_string(DRAWING_SHA256),
            json_string(
                "Complete native Model table/row/cell/content construction, native source text/optional font-size/margin setters, actual TableLayout constructor/TextManager and cold Measure/warm Layout execute through genuine Widget/Content/Text and NAME font selection, bundled Minikin/HarfBuzz/Skia/FreeType. Caller supplies 2x2 source bounds, physical text, explicit default density/direction interfaces and null caller manager, width1000, identity ARGB context resolution and local split bands through actual setter; bands are not supplied measurements. After warm layout, native RichTextLayout.m_GetMaxCharHeight executes and its exact bits are checked against stored native layout+276; sorted native padding vector+184 is observed per physical child. Actual dense measured/placed text entries, bounds and cached emitted runs are inherited from the shared live-table capture. Native cached getDrawnTextRun executes after each stage including between cold Measure and warm Layout with nil EmojiFontSlices, allocates output objects/vectors retained until reset, and is not a pure observer; source tracing on this captured route finds no measured-entry/frame/source-bounds writes. Pinned host ICU76.1/Unicode16 substitutes Android ICU; host allocation/files/libc/single-thread primitives, deterministic UUIDs, pow/acosf and disabled ATrace are supplied services. Native runtime registries reset; bounded table/layout/input storage is zeroed before actual constructors while owned native allocation fills vary. No saved parser/clone, pagination/split-band production, Bodytext source-rectangle callbacks, final SVG/PDF writer consumption, device pixels or arbitrary font configuration execute."
            ),
            output.join(",")
        );
    });
}
