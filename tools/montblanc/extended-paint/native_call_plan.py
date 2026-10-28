"""Build the exact controlled native-call sequence from a V2 capture record."""


def build_native_call_plan(record, symbols, *, receiver, vector_ptrs, queue,
                           subbitmap, rect, namebuf, matrix, width, height, argb):
    """Validate the real case schema and produce explicit ABI call rows.

    V2 capture records expose synthetic_model_input, not source_input. Keeping
    the schema lookup here makes that boundary testable without running native
    code. Values in this plan are pointers/arguments supplied by the caller;
    only pen size is taken from the pinned record.
    """
    if not isinstance(record, dict):
        raise TypeError("capture case must be an object")
    model = record.get("synthetic_model_input")
    if not isinstance(model, dict):
        raise ValueError("capture case lacks synthetic_model_input")
    size = model.get("pen_size")
    if not isinstance(size, (int, float)) or size <= 0:
        raise ValueError("synthetic model pen_size is missing or invalid")
    if len(vector_ptrs) != 3 or any(not isinstance(x, int) or x <= 0 for x in vector_ptrs):
        raise ValueError("expected three non-null Add-owned vector pointers")
    if width <= 0 or height <= 0:
        raise ValueError("target dimensions must be positive")
    ordered = (
        ("init", symbols["init"], (receiver,), (), ()),
        ("create_canvas", symbols["create_canvas"], (receiver, width, height, queue), (), ()),
        ("subbitmap_ctor", symbols["subbitmap_ctor"],
         (subbitmap, queue, 0, rect, width, height, namebuf, 0), (), (1, 6, 2)),
        ("subbitmap_create_framebuffer", symbols["subbitmap_create_framebuffer"], (subbitmap,), (), ()),
        ("set_pen_data", symbols["set_pen_data"], (receiver, argb), (float(size),), ()),
        ("set_buffer", symbols["set_buffer"], (receiver, *vector_ptrs), (), ()),
        ("send", symbols["send"], (receiver, *vector_ptrs), (), ()),
        ("update", symbols["update"], (receiver, 0), (), ()),
        ("draw", symbols["draw"], (receiver, subbitmap, matrix), (), ()),
    )
    return {
        name: {"symbol": symbol, "regs": regs, "floats": floats,
               "stack": stack, **({"limit": 8_000_000} if name == "draw" else {})}
        for name, symbol, regs, floats, stack in ordered
    }


def execute_call_plan(fake_call, plan):
    """Exercise ordering and ABI tuples with a caller supplied pure stub."""
    for name, row in plan.items():
        fake_call(name, row["symbol"], row["regs"], row["floats"],
                  row["stack"], row.get("limit"))
