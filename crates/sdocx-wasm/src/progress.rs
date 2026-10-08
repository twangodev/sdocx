use sdocx::Progress;
use serde::Serialize;
use wasm_bindgen::JsValue;

pub(crate) fn observer(callback: &js_sys::Function) -> impl FnMut(Progress) + '_ {
    let mut previous: Option<Progress> = None;
    let mut last_sent = f64::NEG_INFINITY;
    move |progress| {
        let now = js_sys::Date::now();
        let transition = previous.is_none_or(|previous| {
            previous.stage != progress.stage || previous.total != progress.total
        });
        let finished = progress.total == Some(progress.completed);
        let started = previous.is_some_and(|previous| {
            previous.stage == progress.stage && previous.completed == 0 && progress.completed > 0
        });
        if transition || started || finished || now - last_sent >= 75.0 {
            if let Ok(value) = progress
                .serialize(&serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true))
            {
                // Progress is observational; a failed listener must not invalidate vector output.
                let _ = callback.call1(&JsValue::UNDEFINED, &value);
            }
            last_sent = now;
        }
        previous = Some(progress);
    }
}
