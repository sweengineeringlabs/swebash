// ---------------------------------------------------------------------------
// Host imports — provided by the native host runtime
// ---------------------------------------------------------------------------

extern "C" {
    // I/O
    pub fn host_write(ptr: *const u8, len: usize);
    pub fn host_write_err(ptr: *const u8, len: usize);

    // Filesystem
    pub fn host_read_file(path_ptr: *const u8, path_len: usize) -> i32;
    pub fn host_list_dir(path_ptr: *const u8, path_len: usize) -> i32;
    pub fn host_stat(path_ptr: *const u8, path_len: usize) -> i32;
    pub fn host_write_file(
        path_ptr: *const u8,
        path_len: usize,
        data_ptr: *const u8,
        data_len: usize,
        append: i32,
    ) -> i32;
    pub fn host_remove(path_ptr: *const u8, path_len: usize, recursive: i32) -> i32;
    pub fn host_copy(
        src_ptr: *const u8,
        src_len: usize,
        dst_ptr: *const u8,
        dst_len: usize,
    ) -> i32;
    pub fn host_rename(
        src_ptr: *const u8,
        src_len: usize,
        dst_ptr: *const u8,
        dst_len: usize,
    ) -> i32;
    pub fn host_mkdir(path_ptr: *const u8, path_len: usize, recursive: i32) -> i32;
    pub fn host_get_cwd() -> i32;
    pub fn host_set_cwd(path_ptr: *const u8, path_len: usize) -> i32;

    // Environment
    pub fn host_get_env(key_ptr: *const u8, key_len: usize) -> i32;
    pub fn host_set_env(
        key_ptr: *const u8,
        key_len: usize,
        val_ptr: *const u8,
        val_len: usize,
    );
    pub fn host_list_env() -> i32;

    // Process
    pub fn host_spawn(data_ptr: *const u8, data_len: usize) -> i32;

    // Workspace sandbox
    pub fn host_workspace(cmd_ptr: *const u8, cmd_len: usize) -> i32;
}
