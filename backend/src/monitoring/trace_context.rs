use std::cell::RefCell;

thread_local! {
    static TRACE_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_trace_id(trace_id: String) {
    TRACE_ID.with(|id| {
        *id.borrow_mut() = Some(trace_id);
    });
}

pub fn get_trace_id() -> Option<String> {
    TRACE_ID.with(|id| id.borrow().clone())
}

pub fn clear_trace_id() {
    TRACE_ID.with(|id| {
        *id.borrow_mut() = None;
    });
}
