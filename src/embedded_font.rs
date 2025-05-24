// 内嵌中文字体
// 这个文件提供了一个内嵌的带有中文字符支持的字体
// 字体数据来源：Source Han Sans SC Regular

/// 返回内嵌字体的字节数据
pub fn get_embedded_font_data() -> &'static [u8] {
    // 嵌入 Source Han Sans SC 字体数据 (OTF格式)
    // 这是一个完整的中文字体，支持所有中文字符
    // 使用这个字体可以确保中文字符在任何平台上正确显示
    include_bytes!("../assets/SourceHanSansSC-Regular.otf")
}
