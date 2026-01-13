mod get_credential;
mod img;
mod pdf;
mod batch;

pub use img::upload_img;
pub use pdf::upload_pdf;
pub use batch::upload_and_convert_pdf;
