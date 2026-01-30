/// Shared host state passed through the Wasmtime store.
pub struct HostState {
    /// Pointer offset of the response buffer inside wasm linear memory.
    pub response_buf_ptr: u32,
    /// Capacity of the response buffer.
    pub response_buf_cap: u32,
}
