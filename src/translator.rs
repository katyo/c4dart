use std::borrow::Cow;
use std::collections::{HashSet, HashMap};
use clang::{Entity, EntityKind, Type, TypeKind};
use log::*;
use crate::{Options, Coder};

#[derive(Debug, Clone)]
pub struct FuncDef {
    name: Option<String>,
    cmt: Option<String>,
    cffi: String,
    dart: String,
}

impl FuncDef {
    fn from_entity(typenames: &HashMap<String, String>, entity: Entity) -> Self {
        let res = entity.get_result_type();
        let args = entity.get_arguments();

        let cffi_res = res.map(|type_| translate_type(typenames, type_, true))
            .unwrap_or("Void".into());
        let dart_res = res.map(|type_| translate_type(typenames, type_, false))
            .unwrap_or("void".into());

        let cffi_args = args.as_ref().map(|args| translate_args(typenames, args.clone(), true))
            .unwrap_or("".into());
        let dart_args = args.map(|args| translate_args(typenames, args, false))
            .unwrap_or("".into());
        
        Self {
            name: entity.get_name(),
            cmt: entity.get_comment(),
            cffi: format!("{res} Function({args})",
                          res = cffi_res,
                          args = cffi_args),
            dart: format!("{res} Function({args})",
                          res = dart_res,
                          args = dart_args),
        }
    }
    
    fn from_type<'a>(typenames: &HashMap<String, String>, type_: Type<'a>) -> Self {
        let res = type_.get_result_type();
        let args = type_.get_argument_types();

        let cffi_res = res.map(|type_| translate_type(typenames, type_, true))
            .unwrap_or("Void".into());
        let dart_res = res.map(|type_| translate_type(typenames, type_, false))
            .unwrap_or("void".into());

        let cffi_args = args.as_ref().map(|args| translate_types(typenames, args.clone(), true))
            .unwrap_or("".into());
        let dart_args = args.map(|args| translate_types(typenames, args, false))
            .unwrap_or("".into());
        
        Self {
            name: None,
            cmt: None,
            cffi: format!("{res} Function({args})",
                          res = cffi_res,
                          args = cffi_args),
            dart: format!("{res} Function({args})",
                          res = dart_res,
                          args = dart_args),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Translator {
    options: Options,

    exported: HashSet<String>,
    typenames: HashMap<String, String>,
    
    calls: Vec<(String, FuncDef)>,
    callbacks: Vec<(String, FuncDef)>,
    
    coder: Coder,
}

impl Translator {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            exported: HashSet::default(),
            typenames: HashMap::default(),
            calls: Vec::default(),
            callbacks: Vec::default(),
            coder: Coder::default(),
        }
    }
    
    pub fn translate(&mut self, entity: Entity) {
        use EntityKind::*;
        
        self.coder.line("import 'dart:ffi';");
        self.coder.line("");

        for entity in entity.get_children() {
            if let Some(name) = entity.get_name() {
                if self.match_name(&name) {
                    match entity.get_kind() {
                        FunctionDecl => self.parse_function(&name, entity),
                        _ => {},
                    }
                }
            }
        }

        for entity in entity.get_children() {
            if let Some(name) = entity.get_name() {
                if self.match_name(&name) {
                    let xname = self.make_name(&name);
                    if self.export_once(&name) {
                        match entity.get_kind() {
                            EnumDecl => self.translate_enum(&name, &xname, entity),
                            _ => {},
                        }
                    }
                }
            }
        }
        
        self.coder.comment("Library class");

        let class = &self.options.class_name;
        let calls = &self.calls;
        let callbacks = &self.callbacks;
        
        self.coder.block(format!("class {name}", name = class), |coder| {
            coder.comment("Callbacks");

            for (name, func) in callbacks {
                if let Some(cmt) = &func.cmt {
                    coder.comment(cmt);
                }
                coder.line(format!("final Pointer<NativeFunction<{type}>> _{name};",
                                   type = func.cffi,
                                   name = name));
            }
            
            coder.comment("Functions");

            for (name, func) in calls {
                if let Some(cmt) = &func.cmt {
                    coder.comment(cmt);
                }
                coder.line(format!("final {type} _{name};",
                                   type = func.dart,
                                   name = name));
            }

            coder.comment("Constructor");
            coder.line(format!("{name}(", name = class));
            coder.line("    DynamicLibrary dylib");
            
            for (name, _func) in callbacks {
                /*coder.line(format!("  , {type} {name}_",
                                   type = func.dart,
                                   name = name));*/
                coder.line(format!("  , this._{name}",
                                   name = name));
            }
            
            coder.line(")");

            let mut initial = true;

            /*coder.comment("Init callbacks");
            for (name, _func) in callbacks {
                coder.line(format!("{sep} _{name} = Pointer.fromFunction({name}_)",
                                   name = name,
                                   sep = if initial { ':' } else { ',' }));
                if initial { initial = false; }
            }*/

            coder.comment("Init functions");            
            for (name, func) in calls {
                coder.line(format!("{sep} _{name} = dylib.lookup<NativeFunction<{type}>>('{ffi_name}').asFunction()",
                                   type = func.cffi,
                                   name = name,
                                   ffi_name = func.name.as_ref().unwrap(),
                                   sep = if initial { ':' } else { ',' }));
                if initial { initial = false; }
            }
            
            coder.line("{}");
        });
    }

    fn parse_function(&mut self, name: &str, entity: Entity) {
        info!("Parse function: `{}`", name);

        let res = entity.get_result_type().unwrap();
        let args = entity.get_arguments().unwrap();

        let xname = self.make_name(name);

        self.parse_type(res);

        let mut num = 0;
        
        for arg in args {
            use TypeKind::*;
            
            let type_ = arg.get_type().unwrap();
            let canonical_type = type_.get_canonical_type();

            if canonical_type.get_kind() == Pointer {
                let type_ = canonical_type.get_pointee_type().unwrap();

                match type_.get_kind() {
                    FunctionPrototype | FunctionNoPrototype => {
                        let name = arg.get_name().unwrap_or_else(|| {
                            let res = format!("cb{}", num);
                            num += 1;
                            res
                        });
                        
                        let xname = format!("{fn_name}_{arg_name}",
                                            fn_name = xname,
                                            arg_name = name);
                        self.callbacks.push((xname, FuncDef::from_type(&self.typenames, type_)));
                        continue;
                    }
                    _ => {}
                }
            }
                    
            self.parse_type(type_);
        }

        self.calls.push((xname, FuncDef::from_entity(&self.typenames, entity)));
    }

    fn parse_type<'a>(&mut self, type_: Type<'a>) {
        use TypeKind::*;
        use EntityKind::*;
        
        match type_.get_kind() {
            Pointer => self.parse_type(type_.get_pointee_type().unwrap()),
            _ => if let Some(entity) = type_.get_declaration() {
                trace!("parse type: {:?}", entity);
                if let Some(name) = entity.get_name() {
                    let xname = self.make_name(&name);
                    if !self.exported.contains(&name) {
                        match entity.get_kind() {
                            EnumDecl => self.translate_enum(&name, &xname, entity),
                            StructDecl => self.translate_struct(&name, &xname, entity),
                            TypedefDecl => if !self.translate_typedef(&name, &xname, entity) {
                                warn!("Unparsed typedef: {:?}", entity);
                                return;
                            }
                            _ => {
                                warn!("Unparsed typedecl: {:?}", entity);
                                return;
                            }
                        }
                        self.exported.insert(name.clone());
                        self.typenames.insert(name, xname);
                    }
                }
            }
        }
    }

    pub fn coder(&self) -> &Coder {
        &self.coder
    }

    fn match_name(&self, name: impl AsRef<str>) -> bool {
        self.options.names_match.is_match(name.as_ref())
    }

    fn make_name(&self, name: impl AsRef<str>) -> String {
        self.options.names_match.replace(name.as_ref(), &self.options.names_replace as &str).into()
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

    fn translate_typedef(&mut self, name: &str, xname: &str, entity: Entity) -> bool {
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
            }
            _ => {
                warn!("Untranslated typedef {:?}: `{}` as `{}`", type_, name, xname);
                return false;
            }
        }

        true
    }
}

fn translate_type(typenames: &HashMap<String, String>, type_: Type<'_>, ffi: bool) -> Cow<'static, str> {
    use TypeKind::*;

    let canonical_type = type_.get_canonical_type();
    
    debug!("Translate type: {:?} canonical: {:?}", type_, canonical_type);
    
    let kind = canonical_type.get_kind();
    
    if let Some(type_) = if ffi { cffi_type(kind) } else { dart_type(kind) } {
        return type_.into();
    }
    
    match kind {
        Pointer => {
            let type_ = type_.get_pointee_type()
                .or_else(|| canonical_type.get_pointee_type())
                .unwrap();
            format!("Pointer<{}>", translate_type(typenames, type_, true)).into()
        }
        Record => {
            let decl = type_.get_declaration().unwrap();
            let name = decl.get_name().unwrap();

            if let Some(name) = typenames.get(&name) {
                name.clone().into()
            } else {
                name.into()
            }
        }
        FunctionPrototype | FunctionNoPrototype => {
            let cb = FuncDef::from_type(typenames, canonical_type);
            format!("NativeFunction<{}>", cb.cffi).into()
        }
        kind => {
            error!("Unsupported type kind: {:?}", kind);
            format!("<unsupported_type_kind:{:?}>", kind).into()
        }
    }
}

fn translate_types<'a>(typenames: &HashMap<String, String>, types: impl IntoIterator<Item = Type<'a>>, ffi: bool) -> String {
    types.into_iter().map(|type_| translate_type(typenames, type_, ffi))
        .collect::<Vec<_>>().join(", ")
}

fn translate_args<'a>(typenames: &HashMap<String, String>, args: impl IntoIterator<Item = Entity<'a>>, ffi: bool) -> String {
    args.into_iter().map(|arg| {
        let type_ = arg.get_type().unwrap();
        let type_ = translate_type(typenames, type_, ffi);
        
        if let Some(name) = arg.get_name() {
            format!("{type} {name}", type = type_, name = name).into()
        } else {
            type_
        }
    }).collect::<Vec<_>>().join(", ")
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
        Float | Double => "double".into(),
        _ => return None,
    })
}
