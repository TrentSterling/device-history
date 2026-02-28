fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if std::path::Path::new("assets/icon.ico").exists() {
            res.compile().unwrap();
        }
    }
}
