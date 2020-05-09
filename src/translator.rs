use std::borrow::Cow;
use std::collections::{HashSet, HashMap};
use clang::{Entity, EntityKind, Type, TypeKind};
use log::*;
use crate::{Options, Coder};

#[derive(Debug, Clone)]
pub struct Translator {
    options: Options,
    exported: HashSet<String>,
    funcs: HashMap<String, String>,
    func_types: HashMap<String, String>,
    coder: Coder,
}

impl Translator {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            exported: HashSet::default(),
            funcs: HashMap::default(),
            func_types: HashMap::default(),
            coder: Coder::default(),
        }
    }
    
    pub fn translate(&mut self, entity: Entity) {
        self.coder.line("import 'dart:ffi';");
        self.coder.line("");
        
        for entity in entity.get_children() {
            use EntityKind::*;

            if let Some(name) = entity.get_name() {
                if self.match_name(&name) {
                    let xname = self.make_name(&name);
                    if self.export_once(&xname) {
                        match entity.get_kind() {
                            EnumDecl => self.translate_enum(&name, &xname, entity),
                            StructDecl => self.translate_struct(&name, &xname, entity),
                            TypedefDecl => self.translate_typedef(&name, &xname, entity),
                            FunctionDecl => self.translate_function(&name, &xname, entity),
                            _ => warn!("Untranslated entity: {:?}", entity),
                        }
                    }
                }
            }
        }
    }

    pub fn make_class(&mut self, class: impl AsRef<str>) {
        let class = class.as_ref();
        let funcs = &self.funcs;
        let func_types = &self.func_types;

        self.coder.comment("Library class");
        self.coder.block(format!("class {name}", name = class), |coder| {
            coder.comment("Functions");
            for (_, xname) in funcs {
                coder.line(format!("final {name} _{name};", name = xname));
            }
            coder.comment("Callbacks");
            for (_, xname) in func_types {
                coder.line(format!("final {name}_ _{name};", name = xname));
            }

            coder.line(format!("{name}(", name = class));
            coder.line("    DynamicLibrary dylib");
            for (_, xname) in func_types {
                coder.line(format!("  , {name} {name}_", name = xname));
            }
            coder.line(")");

            let mut initial = true;

            coder.comment("Init functions");
            for (_, xname) in funcs {
                coder.line(format!("{sep} _{name} = dylib.lookup<NativeFunction<{name}>>('{name}').asFunction()",
                                   name = xname, sep = if initial { ':' } else { ',' }));
                if initial { initial = false; }
            }
            coder.comment("Init callbacks");
            for (_, xname) in func_types {
                coder.line(format!("{sep} _{name} = Pointer.fromFunction({name}_)",
                                   name = xname, sep = if initial { ':' } else { ',' }));
                if initial { initial = false; }
            }
            coder.line("{}");
        });
    }

    pub fn coder(&self) -> &Coder {
        &self.coder
    }

    fn match_name(&self, name: impl AsRef<str>) -> bool {
        self.options.match_.is_match(name.as_ref())
    }

    fn make_name(&self, name: impl AsRef<str>) -> String {
        self.options.match_.replace(name.as_ref(), &self.options.replace as &str).into()
    }

    fn export_once(&mut self, name: impl AsRef<str>) -> bool {
        let name = name.as_ref();
        if self.exported.contains(name) {
            false
        } else {
            self.exported.insert(name.into());
            true
        }
    }

    fn translate_enum(&mut self, name: &str, xname: &str, entity: Entity) {
        info!("Translate enum: `{}` as `{}`", name, xname);

        if let Some(cmt) = entity.get_comment() {
            self.coder.comment(cmt);
        }
        self.coder.block(format!("class {name}",
                                 name = xname), |coder| {
            for entity in entity.get_children() {
                if entity.get_kind() == EntityKind::EnumConstantDecl {
                    let ent_name = entity.get_name().unwrap();
                    let ent_name = without_prefix(ent_name, &name);
                    
                    let ent_val = entity.get_enum_constant_value().unwrap().0;
                    
                    coder.line(format!("static const {name} = {value};",
                                       name = ent_name,
                                       value = ent_val));
                }
            }
        });
    }

    fn translate_field(coder: &mut Coder, entity: Entity) {
        if entity.get_kind() == EntityKind::FieldDecl {
            let name = entity.get_name().unwrap();
            let type_ = entity.get_type().unwrap();

            info!("Translate field: `{}` of type `{:?}`", name, type_);
            
            let ffi_type = type_annotation(type_);
            let native_type = native_type(type_);

            if let Some(cmt) = entity.get_comment() {
                coder.comment(cmt);
            }
            coder.line(format!("{ffi_type} {native_type} {name};",
                               name = name,
                               ffi_type = ffi_type,
                               native_type = native_type));
        }
    }
    
    fn translate_struct(&mut self, name: &str, xname: &str, entity: Entity) {
        info!("Translate struct: `{}` as `{}`", name, xname);

        if let Some(cmt) = entity.get_comment() {
            self.coder.comment(cmt);
        }
        self.coder.block(format!("class {name} extends Struct",
                                 name = xname), |coder| {
            for field in entity.get_children() {
                Self::translate_field(coder, field);
            }
        });
    }

    fn translate_typedef(&mut self, name: &str, xname: &str, entity: Entity) {
        use TypeKind::*;
        
        let type_ = entity.get_typedef_underlying_type().unwrap();
        let type_ = type_.get_canonical_type();

        match type_.get_kind() {
            Record => {
                info!("Translate typedef record: `{}` as `{}`", name, xname);

                if let Some(cmt) = entity.get_comment() {
                    self.coder.comment(cmt);
                }
                self.coder.block(format!("class {name} extends Struct",
                                         name = xname), |coder| {
                    for field in type_.get_fields().unwrap() {
                        Self::translate_field(coder, field);
                    }
                });
            },
            Pointer => {
                let type_ = type_.get_pointee_type().unwrap();
                match type_.get_kind() {
                    FunctionPrototype => {
                        info!("Translate typedef function pointer: `{}` as `{}`", name, xname);
                        
                        let res = type_.get_result_type().unwrap();
                        let args = type_.get_argument_types().unwrap();

                        if let Some(cmt) = entity.get_comment() {
                            self.coder.comment(cmt);
                        }
                        self.coder.line(format!("typedef {name}_ = {res} Function({args});",
                                                name = xname,
                                                res = self.translate_type(res, true),
                                                args = self.translate_types(args.clone(), true)));
                        self.coder.line(format!("typedef {name} = {res} Function({args});",
                                                name = xname,
                                                res = self.translate_type(res, false),
                                                args = self.translate_types(args, false)));
                        self.func_types.insert(name.into(), xname.into());
                    },
                    _ => {},
                }
            },
            _ => warn!("Untranslated typedef {:?}: `{}` as `{}`", type_, name, xname),
        }
    }

    fn translate_function(&mut self, name: &str, xname: &str, entity: Entity) {
        info!("Translate function: `{}` as `{}`", name, xname);

        let res = entity.get_result_type().unwrap();
        let args = entity.get_arguments().unwrap();
        
        if let Some(cmt) = entity.get_comment() {
            self.coder.comment(cmt);
        }
        self.coder.line(format!("typedef {name}_ = {res} Function({args});",
                                name = xname,
                                res = self.translate_type(res, true),
                                args = self.translate_args(args.clone(), true)));
        self.coder.line(format!("typedef {name} = {res} Function({args});",
                                name = xname,
                                res = self.translate_type(res, false),
                                args = self.translate_args(args, false)));
        self.funcs.insert(name.into(), xname.into());
    }

    fn translate_type(&self, type_: Type<'_>, ffi: bool) -> Cow<'static, str> {
        use TypeKind::*;

        let canonical_type = type_.get_canonical_type();

        debug!("Translate type: {:?} canonical: {:?}", type_, canonical_type);

        let kind = canonical_type.get_kind();

        if let Some(type_) = if ffi { cffi_type(kind) } else { dart_type(kind) } {
            return type_.into();
        }
        
        match kind {
            Pointer => {
                if let Some(type_) = type_.get_pointee_type() {
                    format!("Pointer<{}>", self.translate_type(type_, ffi)).into()
                } else {
                    let name = type_.get_declaration().unwrap().get_name().unwrap();
                    if self.func_types.contains_key(&name) {
                        let xname = self.make_name(&name);
                        format!("Pointer<NativeFunction<{}>>", xname).into()
                    } else {
                        error!("Unsupported pointer type: {:?}", type_);
                        format!("<unsupported_pointer:{}>", name).into()
                    }
                }
            },
            Record => {
                let decl = type_.get_declaration().unwrap();
                decl.get_name().unwrap().into()
            },
            kind => {
                error!("Unsupported type kind: {:?}", kind);
                format!("<unsupported_type_kind:{:?}>", kind).into()
            },
        }
    }

    fn translate_types<'a>(&self, types: impl IntoIterator<Item = Type<'a>>, ffi: bool) -> String {
        types.into_iter().map(|type_| self.translate_type(type_, ffi))
            .collect::<Vec<_>>().join(", ")
    }

    fn translate_args<'a>(&self, args: impl IntoIterator<Item = Entity<'a>>, ffi: bool) -> String {
        args.into_iter().map(|arg| {
            let type_ = arg.get_type().unwrap();
            let type_ = self.translate_type(type_, ffi);
            
            if let Some(name) = arg.get_name() {
                format!("{type} {name}", type = type_, name = name).into()
            } else {
                type_
            }
        }).collect::<Vec<_>>().join(", ")
    }
}

fn without_prefix(src: impl AsRef<str>, pfx: impl AsRef<str>) -> String {
    let src = src.as_ref();
    let pfx = pfx.as_ref();
    if src.starts_with(pfx) {
        let mut src = &src[pfx.len()..];
        while src.starts_with('_') {
            src = &src[1..];
        }
        src
    } else {
        src
    }.into()
}

fn type_annotation(type_: Type<'_>) -> String {
    let type_ = type_.get_canonical_type();
    if let Some(type_) = cffi_type(type_.get_kind()) {
        format!("@{}()", type_)
    } else {
        "".into()
    }
}

fn native_type(type_: Type<'_>) -> &'static str {
    let type_ = type_.get_canonical_type();
    if let Some(type_) = dart_type(type_.get_kind()) {
        type_
    } else {
        ""
    }
}

fn cffi_type(type_kind: TypeKind) -> Option<&'static str> {
    use TypeKind::*;
    
    Some(match type_kind {
        Void => "Void".into(),
        Bool => "Uint8".into(),
        SChar => "Int8".into(),
        CharS => "Int8".into(),
        UChar => "Uint8".into(),
        Short => "Int16".into(),
        UShort => "Uint16".into(),
        Int => "Int32".into(),
        UInt => "Uint32".into(),
        Long => "Int64".into(),
        ULong => "Uint64".into(),
        Float => "Float".into(),
        Double => "Double".into(),
        _ => return None,
    })
}

fn dart_type(type_kind: TypeKind) -> Option<&'static str> {
    use TypeKind::*;
    
    Some(match type_kind {
        Void => "void".into(),
        Bool |
        SChar | CharS | UChar |
        Short | UShort |
        Int | UInt |
        Long | ULong => "int".into(),
        Float => "float".into(),
        Double => "double".into(),
        _ => return None,
    })
}

/*fn cffi_type(type_: impl AsRef<str>) -> Option<&'static str> {
    Some(match type_.as_ref() {
        "char" | "signed char" => "Int8",
        "unsigned char" => "Uint8",
        "short" | "signed short" => "Int16",
        "unsigned short" => "Uint16",
        "int" | "signed int" => "Int32",
        "unsigned int" => "Uint32",
        "long" | "signed long" => "Int32",
        "unsigned long" => "Uint32",
        "long long" | "signed long long" => "Int64",
        "unsigned long long" => "Uint64",
        "float" => "Float",
        "double" => "Double",
        
        "int8_t" => "Int8",
        "uint8_t" => "Uint8",
        "int16_t" => "Int16",
        "uint16_t" => "Uint16",
        "int32_t" => "Int32",
        "uint32_t" => "Uint32",
        "int64_t" => "Int64",
        "uint64_t" => "Uint64",

        "__int8_t" => "Int8",
        "__uint8_t" => "Uint8",
        "__int16_t" => "Int16",
        "__uint16_t" => "Uint16",
        "__int32_t" => "Int32",
        "__uint32_t" => "Uint32",
        "__int64_t" => "Int64",
        "__uint64_t" => "Uint64",

        _ => return None,
    })
}*/
