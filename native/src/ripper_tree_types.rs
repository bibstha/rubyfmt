#![allow(warnings)]
use serde::*;
use serde_json::Value;

use crate::types::LineNumber;

macro_rules! def_tag {
    ($tag_name:ident) => {
        def_tag!($tag_name, stringify!($tag_name));
    };

    ($tag_name:ident, $tag:expr) => {
        #[derive(Serialize, Debug)]
        pub struct $tag_name;

        impl<'de> Deserialize<'de> for $tag_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct TagVisitor;

                impl<'de> de::Visitor<'de> for TagVisitor {
                    type Value = ();

                    fn expecting(
                        &self,
                        f: &mut std::fmt::Formatter<'_>,
                    ) -> Result<(), std::fmt::Error> {
                        write!(f, $tag)
                    }

                    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
                    where
                        E: de::Error,
                    {
                        if s == $tag {
                            Ok(())
                        } else {
                            Err(E::custom("mismatched tag"))
                        }
                    }
                }

                let tag = deserializer.deserialize_str(TagVisitor)?;
                Ok($tag_name)
            }
        }
    };
}

def_tag!(program_tag, "program");
#[derive(Deserialize, Debug)]
pub struct Program(pub program_tag, pub Vec<Expression>);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Expression {
    Def(Def),
    BodyStmt(BodyStmt),
    VCall(VCall),
    Ident(Ident),
    Params(Params),
    MethodCall(MethodCall),
    DotCall(DotCall),
    MethodAddArg(MethodAddArg),
    Int(Int),
    BareAssocHash(BareAssocHash),
    Symbol(Symbol),
    DynaSymbol(DynaSymbol),
    Call(Call),
    V(Vec<Value>),
}

// isn't parsable, but we do create it in our "normalized tree"
def_tag!(method_call_tag, "method_call");
#[derive(Deserialize, Debug)]
pub struct MethodCall(
    pub method_call_tag,
    pub Vec<CallChainElement>,
    pub Box<Expression>,
    pub bool,
    pub Vec<Expression>,
);

impl MethodCall {
    pub fn new(
        chain: Vec<CallChainElement>,
        method: Box<Expression>,
        use_parens: bool,
        args: Vec<Expression>,
    ) -> Self {
        MethodCall(method_call_tag, chain, method, use_parens, args)
    }
}

def_tag!(def_tag, "def");
#[derive(Deserialize, Debug)]
pub struct Def(pub def_tag, pub Ident, pub Params, pub BodyStmt);

def_tag!(bodystmt_tag, "bodystmt");
#[derive(Deserialize, Debug)]
pub struct BodyStmt(
    pub bodystmt_tag,
    pub Vec<Expression>,
    pub Option<Box<Expression>>,
    pub Option<Box<Expression>>,
    pub Option<Box<Expression>>,
);

def_tag!(vcall);
#[derive(Deserialize, Debug)]
pub struct VCall(vcall, pub Box<Expression>);

def_tag!(ident_tag, "@ident");
#[derive(Deserialize, Debug)]
pub struct Ident(pub ident_tag, pub String, pub LineCol);

impl Ident {
    pub fn line_number(&self) -> LineNumber {
        (self.2).0
    }
}

def_tag!(params_tag, "params");
#[derive(Deserialize, Debug)]
pub struct Params(
    pub params_tag,
    pub Value,
    pub Value,
    pub Value,
    pub Value,
    pub Value,
    pub Value,
    pub Value,
);

#[derive(Deserialize, Debug)]
pub struct LineCol(pub LineNumber, pub u64);

def_tag!(dotCall, "call");
#[derive(Deserialize, Debug)]
pub struct DotCall(pub dotCall);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum CallExpr {
    FCall(FCall),
    Call(Call),
    Expression(Box<Expression>),
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum CallChainElement {
    Expression(Box<Expression>),
    Dot(DotType),
}

def_tag!(method_add_arg_tag, "method_add_arg");
#[derive(Deserialize, Debug)]
pub struct MethodAddArg(pub method_add_arg_tag, pub CallExpr, pub ArgNode);

pub fn normalize_inner_call(call_expr: CallExpr) -> (Vec<CallChainElement>, Box<Expression>) {
    match call_expr {
        CallExpr::FCall(FCall(_, i)) => (vec![], Box::new(Expression::Ident(i))),
        CallExpr::Call(Call(_, left, dot, right)) => {
          let (mut chain, method) = normalize_inner_call(CallExpr::Expression(left));
          chain.push(CallChainElement::Expression(method));
          chain.push(CallChainElement::Dot(dot));
          (chain, right)
        }
        CallExpr::Expression(e) => {
          (Vec::new(), e)
        }
    }
}

pub fn normalize_arg_paren(ap: ArgParen) -> Vec<Expression> {
    match *ap.1 {
        ArgNode::Null(_) => vec![],
        ae => normalize_args(ae),
    }
}

pub fn normalize_args_add_block(aab: ArgsAddBlock) -> Vec<Expression> {
    // .1 is expression list
    // .2 is block
    match aab.2 {
        MaybeBlock::NoBlock(_) => aab.1,
    }
}

pub fn normalize_args(arg_node: ArgNode) -> Vec<Expression> {
    match arg_node {
        ArgNode::ArgParen(ap) => normalize_arg_paren(ap),
        ArgNode::ArgsAddBlock(aab) => normalize_args_add_block(aab),
        ArgNode::Null(_) => panic!("should never be called with null"),
    }
}

impl MethodAddArg {
    pub fn to_method_call(self) -> MethodCall {
        let (chain, name) = normalize_inner_call(self.1);
        let args = normalize_args(self.2);
        MethodCall::new(chain, name, true, args)
    }
}

def_tag!(fcall_tag, "fcall");
#[derive(Deserialize, Debug)]
pub struct FCall(pub fcall_tag, pub Ident);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ArgNode {
    ArgParen(ArgParen),
    ArgsAddBlock(ArgsAddBlock),
    Null(Option<String>),
}

def_tag!(arg_paren_tag, "arg_paren");
#[derive(Deserialize, Debug)]
pub struct ArgParen(pub arg_paren_tag, pub Box<ArgNode>);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum MaybeBlock {
    NoBlock(bool),
}

def_tag!(args_add_block_tag, "args_add_block");
#[derive(Deserialize, Debug)]
pub struct ArgsAddBlock(pub args_add_block_tag, pub Vec<Expression>, pub MaybeBlock);

def_tag!(int_tag, "@int");
#[derive(Deserialize, Debug)]
pub struct Int(pub int_tag, pub String, pub LineCol);

def_tag!(bare_assoc_hash_tag, "bare_assoc_hash");
#[derive(Deserialize, Debug)]
pub struct BareAssocHash(pub bare_assoc_hash_tag, pub Vec<AssocNewOrAssocSplat>);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum AssocNewOrAssocSplat {
    AssocNew(AssocNew),
    AssocSplat(AssocSplat),
}

def_tag!(assoc_new_tag, "assoc_new");
#[derive(Deserialize, Debug)]
pub struct AssocNew(
    pub assoc_new_tag,
    pub LabelOrSymbolLiteralOrDynaSymbol,
    pub Expression,
);

def_tag!(assoc_splat_tag, "assoc_splat");
#[derive(Deserialize, Debug)]
pub struct AssocSplat(pub assoc_splat_tag, pub Ident);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum LabelOrSymbolLiteralOrDynaSymbol {
    Label(Label),
    SymbolLiteral(SymbolLiteral),
    DynaSymbol(DynaSymbol),
}

def_tag!(label_tag, "@label");
#[derive(Deserialize, Debug)]
pub struct Label(pub label_tag, pub String, pub LineCol);

def_tag!(symbol_literal_tag, "symbol_literal");
#[derive(Deserialize, Debug)]
pub struct SymbolLiteral(pub symbol_literal_tag, pub Symbol);

def_tag!(symbol_tag, "symbol_literal");
#[derive(Deserialize, Debug)]
pub struct Symbol(pub symbol_tag, pub Ident);

def_tag!(dyna_symbol_tag, "dyna_symbol");
#[derive(Deserialize, Debug)]
pub struct DynaSymbol(pub dyna_symbol_tag, pub StringContent);

def_tag!(string_content_tag, "string_content");
#[derive(Deserialize, Debug)]
pub struct StringContent(pub string_content_tag, pub StringContentPart);

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum StringContentPart {
    TStringContent(TStringContent),
    StringEmbexpr(StringEmbexpr),
}

def_tag!(tstring_content_tag, "@tstring_content");
#[derive(Deserialize, Debug)]
pub struct TStringContent(pub tstring_content_tag, pub String, pub LineCol);

def_tag!(string_embexpr_tag, "string_embexpr");
#[derive(Deserialize, Debug)]
pub struct StringEmbexpr(pub string_embexpr_tag, pub Box<Expression>, pub LineCol);

def_tag!(call_tag, "call");
#[derive(Deserialize, Debug)]
pub struct Call(pub call_tag, pub Box<Expression>, pub DotType, pub Box<Expression>);

impl Call {
    pub fn to_method_call(self) -> MethodCall {
        let (chain, method) = normalize_inner_call(CallExpr::Call(self));
        MethodCall::new(
          chain,
          method,
          false,
          Vec::new(),
        )
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum DotType {
    Dot(Dot),
    LonleyOperator(LonelyOperator),
}

def_tag!(dot_tag, ".");
#[derive(Deserialize, Debug)]
pub struct Dot(pub dot_tag);

def_tag!(lonely_operator_tag, "&.");
#[derive(Deserialize, Debug)]
pub struct LonelyOperator(pub lonely_operator_tag);
