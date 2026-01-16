# RFC004: Function Signature Notation

**Status**: Draft | **Type**: Feature | **Target**: v0.3.1  
**Topic**: Grammar: Function Signatures, MCP Integration, Parameter Typing

## Summary

This RFC proposes a cleaner function signature syntax with inline parameter declarations, replacing verbose `@arg` attributes with a familiar function-style notation: `deploy(env, version = "latest")`.

## Motivation

The current `@arg` attribute system is verbose and duplicative:

```bash
# @desc Deploy application to environment
# @arg 1:environment string Target environment (staging|prod)
# @arg 2:version string Version to deploy (default: latest)
deploy() {
    ./scripts/deploy.sh $1 ${2:-latest}
}
```

A cleaner approach uses inline parameters:

```bash
# @desc Deploy application to environment
deploy(environment, version = "latest") {
    ./scripts/deploy.sh $environment $version
}
```

## Design Considerations

### Grammar Disambiguation

**Problem**: `name(x, y) command` looks identical to a function call followed by a command.

**Solution**: Function definitions are distinguished by what follows the parameter list:
- If followed by `{` (block) → function definition
- If followed by a command on the same line → function definition  
- If followed by newline or nothing → function call

The grammar uses ordered alternatives where `function_def` is tried before `function_call`, and `function_def` requires either a block or command to follow.

### Polyglot Interpreter Safety

**Problem**: When using `@shell python` or shebang, the function body may contain native function definitions like `def foo(x):` that could confuse the parser.

**Solution**: The Runfile parser only parses the *outer* function signature. Block content between `{ }` is captured as raw text and passed to the interpreter unchanged. The `block_content` rule already handles this correctly by treating everything between braces as opaque content.

### Parameter Descriptions for MCP

**Problem**: The new syntax loses `@arg`'s description capability, resulting in empty descriptions in MCP tool schemas.

**Solution**: Support a hybrid approach where `@arg` can provide descriptions for parameters defined in the signature:

```bash
# @desc Deploy application to environment
# @arg environment Target environment (staging|prod)
# @arg version Version to deploy
deploy(environment, version = "latest") {
    ./scripts/deploy.sh $environment $version
}
```

When both signature params and `@arg` exist:
- Signature defines parameter names, types, and defaults
- `@arg` (without position prefix) provides descriptions only
- Position-based `@arg` (e.g., `1:name`) is ignored when signature params exist

### Default Value Parsing

**Problem**: The naive `(!("," | ")") ~ ANY)+` pattern cannot handle quoted strings containing commas or parentheses.

**Solution**: Use a more sophisticated grammar that handles quoted defaults:

```pest
default_value = @{ 
    "\"" ~ ("\\\"" | (!"\"" ~ ANY))* ~ "\""   // Quoted string
    | "'" ~ ("\\'" | (!"'" ~ ANY))* ~ "'"     // Single-quoted string
    | (!(WHITESPACE* ~ ("," | ")" | NL)) ~ ANY)+  // Unquoted value
}
```

### Variadic Parameters

**Problem**: No way to express functions that accept variable arguments (`$@`).

**Solution**: Support rest parameters with `...` prefix:

```bash
# @desc Echo all arguments
echo_all(...args) {
    echo "All args: $args"
}

# Or mixed with regular params
deploy(environment, ...extra_flags) {
    ./scripts/deploy.sh $environment $extra_flags
}
```

Rest parameters:
- Must be last in the parameter list
- Cannot have a default value
- Are substituted for both `$name` and `$@`
- Marked as not required in MCP schema with `"type": "array"`

### Runtime Type Validation

**Question**: Should `replicas: int` validate at runtime?

**Decision**: Types are used for:
1. MCP JSON schema generation (primary use case)
2. Documentation/self-describing functions
3. Optional runtime validation (warn, don't fail)

Runtime behavior:
- If a value doesn't match the declared type, emit a warning but continue
- This maintains shell scripting's permissive nature while providing hints

---

## Implementation Plan

### Phase 1: Grammar Update

**Update `src/grammar.pest`:**

```pest
// Function definition with optional parameters
// Ordered alternatives: try param_list variants first, then empty parens, then keyword-only
function_def = {
    "function" ~ identifier ~ param_list ~ (block | command)
    | "function" ~ identifier ~ "(" ~ ")" ~ (block | command)
    | "function" ~ identifier ~ (block | command)
    | identifier ~ param_list ~ (block | command)
    | identifier ~ "(" ~ ")" ~ (block | command)
}

// Parameter list: (arg1, arg2: int, arg3: str = "default")
// Must have at least one param (empty parens handled separately)
param_list = { "(" ~ params ~ ")" }
params = { param ~ ("," ~ param)* }
param = { rest_param | regular_param }
rest_param = { "..." ~ identifier }
regular_param = { identifier ~ param_type_annotation? ~ param_default? }
param_type_annotation = { ":" ~ param_type }
param_type = { "int" | "str" | "bool" | "string" | "integer" | "boolean" }
param_default = { "=" ~ default_value }
default_value = @{ 
    "\"" ~ ("\\\"" | (!"\"" ~ ANY))* ~ "\""   // Double-quoted string
    | "'" ~ ("\\'" | (!"'" ~ ANY))* ~ "'"     // Single-quoted string  
    | (!(WHITESPACE* ~ ("," | ")" | NL)) ~ ANY)+  // Unquoted value
}
```

### Phase 2: AST Update

**Update `src/ast.rs`:**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: ArgType,
    pub default_value: Option<String>,
    pub is_rest: bool,  // true for ...args style parameters
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // ... existing variants ...
    
    SimpleFunctionDef {
        name: String,
        params: Vec<Parameter>,  // NEW
        command_template: String,
        attributes: Vec<Attribute>,
    },
    BlockFunctionDef {
        name: String,
        params: Vec<Parameter>,  // NEW
        commands: Vec<String>,
        attributes: Vec<Attribute>,
        shebang: Option<String>,
    },
    // ... other variants ...
}
```

### Phase 3: Parser Implementation

**Update `src/parser.rs`:**

```rust
fn parse_statement(pair: pest::iterators::Pair<Rule>, original_input: &str) -> Option<Statement> {
    match pair.as_rule() {
        Rule::function_def => {
            let span = pair.as_span();
            let line_num = original_input[..span.start()].lines().count();
            let attributes = parse_attributes_from_lines(original_input, line_num);
            
            let mut inner = pair.into_inner();
            let name = inner.next()?.as_str().to_string();
            
            // Check if next element is param_list
            let (params, body_pair) = if let Some(next) = inner.next() {
                if next.as_rule() == Rule::param_list {
                    // Parse parameters
                    let params = parse_param_list(next)?;
                    (params, inner.next()?)
                } else {
                    // No params, this is the body
                    (Vec::new(), next)
                }
            } else {
                return None;
            };

            // Parse body (block or command)
            match body_pair.as_rule() {
                Rule::block => {
                    // ... existing block parsing logic ...
                    Some(Statement::BlockFunctionDef {
                        name,
                        params,  // NEW
                        commands,
                        attributes,
                        shebang,
                    })
                }
                Rule::command => {
                    let command_template = parse_command(body_pair);
                    Some(Statement::SimpleFunctionDef {
                        name,
                        params,  // NEW
                        command_template,
                        attributes,
                    })
                }
                _ => None,
            }
        }
        // ... other rules ...
    }
}

fn parse_param_list(pair: pest::iterators::Pair<Rule>) -> Option<Vec<Parameter>> {
    let mut params = Vec::new();
    
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::params {
            for param_pair in inner.into_inner() {
                if let Some(param) = parse_param(param_pair) {
                    params.push(param);
                }
            }
        }
    }
    
    Some(params)
}

fn parse_param(pair: pest::iterators::Pair<Rule>) -> Option<Parameter> {
    let mut inner = pair.into_inner();
    let first = inner.next()?;
    
    // Check for rest parameter (...args)
    if first.as_rule() == Rule::rest_param {
        let name = first.into_inner().next()?.as_str().to_string();
        return Some(Parameter {
            name,
            param_type: ArgType::String,
            default_value: None,
            is_rest: true,
        });
    }
    
    // Regular parameter
    let name = first.as_str().to_string();
    let mut param_type = ArgType::String;  // Default
    let mut default_value = None;
    
    // Check for type annotation and default value
    for next in inner {
        match next.as_rule() {
            Rule::param_type_annotation => {
                if let Some(type_pair) = next.into_inner().next() {
                    param_type = match type_pair.as_str() {
                        "int" | "integer" => ArgType::Integer,
                        "bool" | "boolean" => ArgType::Boolean,
                        "str" | "string" => ArgType::String,
                        _ => ArgType::String,
                    };
                }
            }
            Rule::param_default => {
                if let Some(default_pair) = next.into_inner().next() {
                    let val = default_pair.as_str().trim();
                    // Strip surrounding quotes if present
                    let val = if (val.starts_with('"') && val.ends_with('"')) 
                              || (val.starts_with('\'') && val.ends_with('\'')) {
                        &val[1..val.len()-1]
                    } else {
                        val
                    };
                    default_value = Some(val.to_string());
                }
            }
            _ => {}
        }
    }
    
    Some(Parameter {
        name,
        param_type,
        default_value,
        is_rest: false,
    })
}
```

### Phase 4: MCP Integration

**Update `src/mcp.rs`:**

```rust
fn extract_function_metadata(
    name: &str,
    attributes: &[Attribute],
    params: &[Parameter],  // NEW
) -> Option<Tool> {
    let mut description = None;
    let mut properties = HashMap::new();
    let mut required = Vec::new();
    
    // Build a map of @arg descriptions (name -> description)
    let mut arg_descriptions: HashMap<String, String> = HashMap::new();
    
    // Get description from attributes and collect @arg descriptions
    for attr in attributes {
        match attr {
            Attribute::Desc(desc) => {
                description = Some(desc.clone());
            }
            Attribute::Arg(arg_meta) => {
                // Store description keyed by name for lookup
                arg_descriptions.insert(arg_meta.name.clone(), arg_meta.description.clone());
            }
            _ => {}
        }
    }
    
    // If we have params, use them (takes precedence over @arg for type/default)
    if !params.is_empty() {
        for param in params.iter() {
            let param_description = arg_descriptions
                .get(&param.name)
                .cloned()
                .unwrap_or_default();
            
            if param.is_rest {
                // Rest parameter: array type, not required
                properties.insert(
                    param.name.clone(),
                    ParameterSchema {
                        param_type: "array".to_string(),
                        description: param_description,
                    },
                );
            } else {
                properties.insert(
                    param.name.clone(),
                    ParameterSchema {
                        param_type: arg_type_to_json_type(&param.param_type),
                        description: param_description,
                    },
                );
                
                // Only required if no default value and not rest
                if param.default_value.is_none() {
                    required.push(param.name.clone());
                }
            }
        }
    } else {
        // Fall back to @arg attributes for backward compatibility
        for attr in attributes {
            if let Attribute::Arg(arg_meta) = attr {
                properties.insert(
                    arg_meta.name.clone(),
                    ParameterSchema {
                        param_type: arg_type_to_json_type(&arg_meta.arg_type),
                        description: arg_meta.description.clone(),
                    },
                );
                required.push(arg_meta.name.clone());
            }
        }
    }
    
    description.map(|desc| Tool {
        name: name.replace(':', "__"),
        description: desc,
        input_schema: InputSchema {
            schema_type: "object".to_string(),
            properties,
            required,
        },
    })
}
```

### Phase 5: Executor Integration

**Update `src/interpreter.rs`:**

```rust
impl Interpreter {
    fn execute_statement(&mut self, statement: Statement) -> Result<(), Box<dyn std::error::Error>> {
        match statement {
            Statement::SimpleFunctionDef {
                name,
                params,  // NEW
                command_template,
                attributes,
            } => {
                if utils::matches_current_platform(&attributes) {
                    self.simple_functions.insert(name.clone(), command_template);
                    self.function_metadata.insert(
                        name,
                        FunctionMetadata { 
                            attributes,
                            shebang: None,
                            params,  // NEW
                        },
                    );
                }
            }
            Statement::BlockFunctionDef { name, params, commands, attributes, shebang } => {
                if utils::matches_current_platform(&attributes) {
                    self.block_functions.insert(name.clone(), commands);
                    self.function_metadata.insert(
                        name,
                        FunctionMetadata { 
                            attributes,
                            shebang: shebang.clone(),
                            params,  // NEW
                        },
                    );
                }
            }
            // ... other cases ...
        }
        Ok(())
    }
    
    // Update substitute_args to use params
    fn substitute_args_with_params(&self, template: &str, args: &[String], params: &[Parameter]) -> String {
        let mut result = template.to_string();
        
        // If we have params, do named substitution
        if !params.is_empty() {
            for (i, param) in params.iter().enumerate() {
                if param.is_rest {
                    // Rest parameter: join all remaining args
                    let rest_args = if i < args.len() {
                        args[i..].join(" ")
                    } else {
                        String::new()
                    };
                    result = result.replace(&format!("${}", param.name), &rest_args);
                    result = result.replace(&format!("${{{}}}", param.name), &rest_args);
                    result = result.replace("$@", &rest_args);
                } else {
                    let value = if i < args.len() {
                        &args[i]
                    } else if let Some(default) = &param.default_value {
                        default
                    } else {
                        eprintln!("Warning: Missing required argument: {}", param.name);
                        ""
                    };
                    
                    // Replace both $name and ${name} and $N
                    result = result.replace(&format!("${}", param.name), value);
                    result = result.replace(&format!("${{{}}}", param.name), value);
                    result = result.replace(&format!("${}", i + 1), value);  // Also support positional
                }
            }
        } else {
            // Fall back to positional substitution
            result = self.substitute_args(template, args);
        }
        
        result
    }
}
```

### Phase 6: Update FunctionMetadata

```rust
#[derive(Clone)]
struct FunctionMetadata {
    attributes: Vec<Attribute>,
    shebang: Option<String>,
    params: Vec<Parameter>,  // NEW
}
```

---

## Example Usage

**Before** (verbose):
```bash
# @desc Deploy application to environment
# @arg 1:environment string Target environment (staging|prod)
# @arg 2:version string Version to deploy (default: latest)
deploy() {
    ./scripts/deploy.sh $1 ${2:-latest}
}
```

**After** (clean):
```bash
# @desc Deploy application to environment
deploy(environment, version = "latest") {
    ./scripts/deploy.sh $environment $version
}
```

**With types**:
```bash
# @desc Scale a service
scale(service, replicas: int = 1) {
    docker compose scale $service=$replicas
}
```

**With descriptions (hybrid)**:
```bash
# @desc Deploy application to environment
# @arg environment Target environment (staging|prod)
# @arg version Version to deploy
deploy(environment, version = "latest") {
    ./scripts/deploy.sh $environment $version
}
```

**With rest parameters**:
```bash
# @desc Run a command in a container
# @arg container Container name
# @arg command Command and arguments to run
docker:exec(container, ...command) {
    docker compose exec $container $command
}
```

---

## Migration Strategy

1. **Keep `@arg` working** for backward compatibility
2. **Precedence**: signature params > `@arg` > positional
3. **Named variables**: `$service` works in addition to `$1`
4. **Hybrid mode**: `@arg` without position provides descriptions for signature params
5. **Parser gracefully handles**: `()` or `(params)` or no parens (existing syntax)

---

## Open Questions

1. **Should we support destructuring?** e.g., `deploy({env, version})` for JSON input
   - *Recommendation*: Not in v1, revisit if MCP use cases demand it

2. **Should type mismatches be errors or warnings?**
   - *Recommendation*: Warnings only, to maintain shell scripting's permissive nature

3. **Should we support union types?** e.g., `port: int | str`
   - *Recommendation*: Not in v1, `str` is sufficient as a catch-all

---

## Test Cases

### Grammar Tests

```rust
#[test]
fn test_function_with_params() {
    let input = "deploy(env, version) echo $env $version";
    let result = parse_script(input).unwrap();
    // Should parse as SimpleFunctionDef with 2 params
}

#[test]
fn test_function_with_typed_params() {
    let input = "scale(service: str, replicas: int) echo $service $replicas";
    let result = parse_script(input).unwrap();
    // Should parse with correct types
}

#[test]
fn test_function_with_defaults() {
    let input = r#"deploy(env, version = "latest") echo $env $version"#;
    let result = parse_script(input).unwrap();
    // Should parse with default value
}

#[test]
fn test_function_with_rest_param() {
    let input = "echo_all(...args) echo $args";
    let result = parse_script(input).unwrap();
    // Should parse with is_rest = true
}

#[test]
fn test_quoted_default_with_comma() {
    let input = r#"test(val = "a,b,c") echo $val"#;
    let result = parse_script(input).unwrap();
    // Default should be "a,b,c" not just "a"
}

#[test]
fn test_empty_parens_still_works() {
    let input = "greet() echo hello";
    let result = parse_script(input).unwrap();
    // Should parse as SimpleFunctionDef with empty params
}
```

This is way cleaner and maintains the "just write code" philosophy while making MCP integration natural!