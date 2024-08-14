use std::process::exit;

use guardian::{
    auth::openid,
    db::mongo::DB,
    logger::Logger,
    web::{services, start_server},
};
use tracing::error;
use tracing_subscriber::filter::LevelFilter;

/* Sass */
// use sass_rs::{Options, OutputStyle};
// use std::fs;

#[tokio::main]
async fn main() {
    /* Sass */
    // // Define the input and output paths
    // let input_path = "static/scss/main.scss";
    // let output_path = "static/css/main.css";

    // // Create the options for the Sass compiler
    // let mut options = Options::default();
    // options.output_style = OutputStyle::Compressed;

    // // Add include paths for Carbon Design System
    // options.include_paths.push("static/scss/@carbon/styles".into());

    // // Compile the SCSS to CSS
    // match sass_rs::compile_file(input_path, options) {
    //     Ok(css) => {
    //         // Write the compiled CSS to the output file
    //         fs::write(output_path, css).expect("Unable to write CSS file");
    //         println!("SCSS compiled successfully to {}", output_path);
    //     }
    //     Err(err) => {
    //         eprintln!("Error compiling SCSS: {}", err);
    //     }
    // }

    //
    //

    if cfg!(debug_assertions) {
        Logger::start(LevelFilter::INFO);
    } else {
        Logger::start(LevelFilter::WARN);
    }

    services::init_once();
    openid::init_once().await;
    if let Err(e) = DB::init_once("guardian").await {
        error!("{e}");
        exit(1);
    }

    let _ = start_server(true).await;
}
