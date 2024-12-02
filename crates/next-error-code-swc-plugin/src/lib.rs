use swc_core::{
    ecma::{ast::*, transforms::testing::test_inline, visit::*},
    plugin::{plugin_transform, proxies::TransformPluginProgramMetadata},
};

fn record_error_and_return_error_code(message: &str) -> String {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();
    message.hash(&mut hasher);
    let code = format!("E{}", hasher.finish());

    #[cfg(not(test))]
    {
        use std::{fs, path};
        const ERROR_CODE_FILE_PATH: &str = "/cwd/error_codes/";

        // Write error message to file if it doesn't exist
        let error_file_path = format!("{}{}.txt", ERROR_CODE_FILE_PATH, code);
        if !path::Path::new(ERROR_CODE_FILE_PATH).exists() {
            fs::create_dir_all(ERROR_CODE_FILE_PATH)
                .unwrap_or_else(|e| panic!("Failed to create errors directory: {}", e));
        }

        if !path::Path::new(&error_file_path).exists() {
            let mut retries = 0;
            while retries < 3 {
                match fs::write(&error_file_path, message) {
                    Ok(_) => break,
                    Err(e) => {
                        if retries == 2 {
                            eprintln!(
                                "Failed to write error message to {} after 3 attempts: {}",
                                error_file_path, e
                            );
                        }
                        retries += 1;
                    }
                }
            }
        }
    }

    code
}

pub struct TransformVisitor;

impl VisitMut for TransformVisitor {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        let mut code: Option<String> = None;
        let mut new_error_expr: Option<&NewExpr> = None;

        if let Expr::New(new_expr) = expr {
            if let Expr::Ident(ident) = &*new_expr.callee {
                if ident.sym.to_string() == "Error" {
                    new_error_expr = Some(new_expr);

                    // Stringify the first argument and record the error code
                    if let Some(args) = &new_expr.args {
                        if let Some(first_arg) = args.first() {
                            fn traverse_expr(expr: &Expr) -> String {
                                match expr {
                                    Expr::Lit(lit) => match lit {
                                        Lit::Str(str_lit) => str_lit.value.to_string(),
                                        _ => "%s".to_string(),
                                    },

                                    Expr::Tpl(tpl) => {
                                        let mut result = String::new();
                                        let mut expr_iter = tpl.exprs.iter();

                                        for (_i, quasi) in tpl.quasis.iter().enumerate() {
                                            result.push_str(&quasi.raw);
                                            if let Some(expr) = expr_iter.next() {
                                                result.push_str(&traverse_expr(expr));
                                            }
                                        }
                                        result
                                    }

                                    Expr::Bin(bin_expr) => {
                                        // Assume binary expression is always add for two strings
                                        format!(
                                            "{}{}",
                                            traverse_expr(&bin_expr.left),
                                            traverse_expr(&bin_expr.right)
                                        )
                                    }

                                    _ => "%s".to_string(),
                                }
                            }

                            let message: String = traverse_expr(&first_arg.expr);
                            code = Some(record_error_and_return_error_code(&message));
                        }
                    }
                }
            }
        }

        if let Some(code) = code {
            if let Some(new_error_expr) = new_error_expr {
                *expr = Expr::Call(CallExpr {
                    span: new_error_expr.span,
                    callee: Callee::Expr(Box::new(Expr::Member(MemberExpr {
                        span: new_error_expr.span,
                        obj: Box::new(Expr::Ident(Ident::new(
                            "Object".into(),
                            new_error_expr.span,
                            Default::default(),
                        ))),
                        prop: MemberProp::Ident("assign".into()),
                    }))),
                    args: vec![
                        ExprOrSpread {
                            spread: None,
                            expr: Box::new(Expr::New(new_error_expr.clone())),
                        },
                        ExprOrSpread {
                            spread: None,
                            expr: Box::new(Expr::Object(ObjectLit {
                                span: new_error_expr.span,
                                props: vec![PropOrSpread::Prop(Box::new(Prop::KeyValue(
                                    KeyValueProp {
                                        key: PropName::Ident("nextjs_internal_error_code".into()),
                                        value: Box::new(Expr::Lit(Lit::Str(Str {
                                            span: new_error_expr.span,
                                            value: code.into(),
                                            raw: None,
                                        }))),
                                    },
                                )))],
                            })),
                        },
                    ],
                    type_args: None,
                    ctxt: new_error_expr.ctxt,
                });
            }
        }
    }
}

#[plugin_transform]
pub fn process_transform(
    mut program: Program,
    _metadata: TransformPluginProgramMetadata,
) -> Program {
    let mut visitor = TransformVisitor;
    visitor.visit_mut_program(&mut program);
    program
}

// An example to test plugin transform.
// Recommended strategy to test plugin's transform is verify
// the Visitor's behavior, instead of trying to run `process_transform` with mocks
// unless explicitly required to do so.

test_inline!(
    Default::default(),
    |_| visit_mut_pass(TransformVisitor),
    realistic_api_handler,
    // Input codes
    r#"
async function fetchUserData(userId) {
    try {
        const response = await fetch(`/api/users/${userId}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch user ${userId}: ${response.statusText}`);
        }
        return await response.json();
    } catch (err) {
        throw new Error(`Request failed: ${err.message}`);
    }
}"#,
    // Output codes after transformed with plugin
    r#"
async function fetchUserData(userId) {
    try {
        const response = await fetch(`/api/users/${userId}`);
        if (!response.ok) {
            throw Object.assign(new Error(`Failed to fetch user ${userId}: ${response.statusText}`), { nextjs_internal_error_code: "E9366994806066179493" });
        }
        return await response.json();
    } catch (err) {
        throw Object.assign(new Error(`Request failed: ${err.message}`), { nextjs_internal_error_code: "E4352108395338836290" });
    }
}"#
);
