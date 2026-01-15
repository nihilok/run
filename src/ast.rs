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
        attributes: Vec<Attribute>,
    },
    BlockFunctionDef {
        name: String,
        commands: Vec<String>,
        attributes: Vec<Attribute>,
        shebang: Option<String>,
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

#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    Os(OsPlatform),
    Shell(ShellType),
    Desc(String),
    Arg(ArgMetadata),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArgMetadata {
    pub position: usize,
    pub name: String,
    pub arg_type: ArgType,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgType {
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OsPlatform {
    Windows,
    Linux,
    MacOS,
    Unix,  // Matches both Linux and MacOS
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShellType {
    Python,
    Python3,
    Node,
    Ruby,
    Pwsh,
    Bash,
    Sh,
}
