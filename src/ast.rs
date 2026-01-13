// Abstract Syntax Tree definitions

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assignment {
        name: String,
        value: Expression,
    },
    SimpleFunctionDef {
        name: String,
        command_template: String,
    },
    BlockFunctionDef {
        name: String,
        commands: Vec<String>,
    },
    FunctionCall {
        name: String,
        args: Vec<String>,
    },
    Command {
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    String(String),
}
