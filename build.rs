//! 构建脚本：为 Windows 可执行文件嵌入版本资源
//!
//! 版本、作者、版权等信息会显示在 exe 的「属性 → 详细信息」中。
//! 其他平台（或交叉编译到 macOS）时直接跳过。

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut res = winres::WindowsResource::new();
    // 图标资源 id 为 1，运行时由托盘 / 任务栏加载
    res.set_icon("assets/icon.ico")
        .set("FileDescription", "高性能跨平台桌面截图软件")
        .set("ProductName", "ScreenCapture")
        .set("CompanyName", "和风")
        .set("LegalCopyright", "© 2026 和风 <dreign@qq.com> · https://hefeng.chat/")
        .set("OriginalFilename", "ScreenCapture.exe")
        .set("ProductVersion", env!("CARGO_PKG_VERSION"))
        .set("FileVersion", env!("CARGO_PKG_VERSION"))
        .set("Comments", "作者：和风 · dreign@qq.com · https://hefeng.chat/");

    if let Err(err) = res.compile() {
        // 资源编译依赖 Windows SDK 的 rc.exe，失败时给出明显提示而不是静默跳过
        println!("cargo:warning=嵌入 Windows 版本资源失败：{err}");
    }
}
