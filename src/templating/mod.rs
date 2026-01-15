use tera::Tera;
use tracing::error;

pub fn start_template_eng() -> Tera {
    match Tera::new("templates/**/*.html") {
        Ok(t) => t,
        Err(e) => {
            error!("Parsing error(s): {}", e);
            ::std::process::exit(1);
        }
    }
}
