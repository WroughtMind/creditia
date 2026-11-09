//! Incremental project-picker marks. Run `fill` on the background executor and
//! forward each result to the UI; opening the picker never reads every archive
//! or waits for a whole library before displaying its first row.
use crate::renderer::{self, Cancellation};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct RowRequest {
    pub id: String,
    pub group: String,
    pub named: bool,
    pub lang: String,
}

/// Reads and renders one immutable project at a time at preload priority. The
/// caller owns generation/stamp checks before publishing a result. Returning
/// false from `emit` stops work when its channel or owning view has gone away.
pub fn fill(
    requests: Vec<RowRequest>,
    cancellation: Cancellation,
    emit: impl FnMut(String, Result<Value, String>) -> bool,
) {
    fill_with(
        requests,
        &cancellation,
        |id| creditia_desktop_service::credits(id).map_err(|error| error.to_string()),
        |input| renderer::request_with_priority(input, cancellation.clone(), 3),
        emit,
    );
}

fn fill_with(
    requests: Vec<RowRequest>,
    cancellation: &Cancellation,
    mut read: impl FnMut(String) -> Result<Value, String>,
    mut render: impl FnMut(Value) -> Result<Value, String>,
    mut emit: impl FnMut(String, Result<Value, String>) -> bool,
) {
    for request in requests {
        if cancellation.cancelled() {
            break;
        }
        let result = read(request.id.clone()).and_then(|raw| {
            if cancellation.cancelled() {
                return Err("cancelled".into());
            }
            render(json!({
                "version":1,"id":"picker-mark","op":"mark","raw":raw,
                "group":request.group,"lang":request.lang,"named":request.named,
            }))
        });
        if cancellation.cancelled() || !emit(request.id, result) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RowRequest, fill_with};
    use crate::renderer::Cancellation;
    use serde_json::json;
    use std::cell::Cell;

    fn request(id: &str) -> RowRequest {
        RowRequest {
            id: id.into(),
            group: "model".into(),
            named: false,
            lang: "en".into(),
        }
    }

    #[test]
    fn missing_archive_does_not_block_later_marks_or_change_options() {
        let mut events = Vec::new();
        fill_with(
            vec![request("missing"), request("ready")],
            &Cancellation::default(),
            |id| match id.as_str() {
                "missing" => Err("no_credits".into()),
                _ => Ok(json!({"project":{"name":id}})),
            },
            |input| {
                assert_eq!(input["raw"]["project"]["name"], "ready");
                assert_eq!(input["group"], "model");
                assert_eq!(input["lang"], "en");
                assert_eq!(input["named"], false);
                Ok(json!({"svg":"ready"}))
            },
            |id, result| {
                events.push((id, result));
                true
            },
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], ("missing".into(), Err("no_credits".into())));
        assert_eq!(events[1].1.as_ref().unwrap()["svg"], "ready");
    }

    #[test]
    fn closing_during_archive_read_cancels_render_and_publication() {
        let cancellation = Cancellation::default();
        let reads = Cell::new(0);
        fill_with(
            vec![request("one"), request("two")],
            &cancellation,
            |_| {
                reads.set(reads.get() + 1);
                cancellation.cancel();
                Ok(json!({}))
            },
            |_| panic!("cancelled picker must not queue a render"),
            |_, _| panic!("cancelled picker must not publish stale marks"),
        );
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn dropped_receiver_stops_before_reading_the_next_archive() {
        let reads = Cell::new(0);
        fill_with(
            vec![request("one"), request("two")],
            &Cancellation::default(),
            |_| {
                reads.set(reads.get() + 1);
                Ok(json!({}))
            },
            |_| Ok(json!({})),
            |_, _| false,
        );
        assert_eq!(reads.get(), 1);
    }
}
