pub mod check_paper_exit;
mod get_request;
mod send_request;
pub mod upload;
pub mod llm;
pub use check_paper_exit::check_paper_name_exist;
pub use get_request::send_api_get_request;
pub use upload::{upload_img, upload_pdf};