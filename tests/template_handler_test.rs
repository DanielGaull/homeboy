use std::error::Error;

use cortex_lang::interpreting::interpreter::CortexInterpreter;
use homeboy::templating::handler::TemplateHandler;

#[test]
fn test_template_loader() -> Result<(), Box<dyn Error>> {
    let mut interpreter = CortexInterpreter::new()?;
    let mut handler = TemplateHandler::new();
    handler.load_from_file("./tests/res/test_template_file.txt", &mut interpreter)?;
    Ok(())
}
