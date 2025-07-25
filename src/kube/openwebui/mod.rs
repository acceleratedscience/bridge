#![allow(dead_code)]

use std::marker::PhantomData;

pub const OWUI: &str = "owui";

// This is a placeholder for openwebui CRD
struct OpenWebUI {
    _p: PhantomData<()>,
}
