use crate::lexer::Lexer;
use crate::token::Token;
use crate::nodes::*;
use crate::error::{ParseResult, ParseError};
use std::iter::{Iterator, Peekable};
use std::rc::Rc;

pub struct AST<'i>
{
    lexer: Peekable<Lexer<'i>>,
}

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
impl<'i> Display for AST<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "AST")
    }
}
impl<'i> Debug for AST<'i> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "AST")
    }
}

impl<'i> AST<'i> {
    pub fn new(input: &'i str) -> ParseResult<AST<'i>> {
        let lexer = Lexer::new(input).peekable();
        Ok(AST {
            lexer,
        })
    }

    pub fn parse(&'i mut self) -> ParseResult<Document> {
        let definitions = self.parse_definitions()?;
        Ok(Document::new(definitions))
    }

    fn parse_description(&mut self) -> ParseResult<Description> {
        match self.unwrap_peeked_token()? {
            Token::BlockStr(_, _, _, _) |
            Token::Str(_, _, _, _) => {
                let tok = self.unwrap_next_token()?;
                Ok(Some(StringValueNode::new(tok)?))
            },
            _ => Ok(None),
        }
    }

    fn parse_argument(&mut self) -> ParseResult<InputValueDefinitionNode> {
        let description = self.parse_description()?;
        let name_tok = self.unwrap_next_token()?;
        let type_node = self.parse_field_type()?;
        let default_value = self.parse_value()?;
        InputValueDefinitionNode::new(name_tok, type_node, description, default_value)
    }

    fn parse_arguments(&mut self) -> ParseResult<Option<Arguments>> {
        match self.expect_optional_token(&Token::OpenParen(0,0,0)) {
            Some(_) => {
                self.unwrap_next_token()?;  // Consume token
                if let Some(_) = self.expect_optional_token(&Token::CloseParen(0,0,0)) {
                    return Err(ParseError::ArgumentEmpty)
                }
                let mut args: Arguments = Vec::new();
                loop {
                    args.push(self.parse_argument()?);
                    if let Some(_) = self.expect_optional_token(&Token::CloseParen(0,0,0)) {
                        break;
                    }
                }
                Ok(None)
            },
            None => Ok(None)
        }
    }

    fn parse_definitions(&'i mut self) -> ParseResult<Vec<DefinitionNode>> {
        self.expect_token(Token::Start)?;
        if let Some(_) = self.expect_optional_token(&Token::End) {
            Err(ParseError::DocumentEmpty)
        } else {
            let mut nodes: Vec<DefinitionNode> = Vec::new();
            loop {
                nodes.push(self.parse_definition()?);
                if let Some(_) = self.expect_optional_token(&Token::End) {
                    break;

                }
            }
            Ok(nodes)
        }
    }

    fn parse_definition(&mut self) -> ParseResult<DefinitionNode> {
        let description = self.parse_description()?;
        let tok = self.unwrap_peeked_token()?;
        if let Token::Name(_, _, _, val) = tok {
            match *val {
                "type" | "enum" => Ok(DefinitionNode::TypeSystem(
                    TypeSystemDefinitionNode::Type(
                        self.parse_type(description)?
                    )
                )),
                _ => Err(ParseError::BadValue),
            }
        } else {
            Err(ParseError::UnexpectedToken {
                expected: String::from("Token<Name>"),
                received: tok.to_string().to_owned(),

            })
        }
    }

    fn parse_type(&mut self, description: Description) -> Result<TypeDefinitionNode,  ParseError> {
        let tok = self.unwrap_next_token()?;
        if let Token::Name(_, _, _, val) = tok {
            match val {
                "type" => Ok(
                    TypeDefinitionNode::Object(
                        self.parse_object_type(description)?
                    )
                ),
                "enum" => Ok(
                    TypeDefinitionNode::Enum(
                        self.parse_enum_type(description)?
                    )
                ),
                _ => Err(ParseError::BadValue),
            }
        } else {
            Err(ParseError::UnexpectedToken {
                expected: String::from("Token::Name"),
                received: tok.to_string().to_owned(),
            })
        }
    }

    fn parse_object_type(&mut self, description: Description) -> ParseResult<ObjectTypeDefinitionNode> {

        let name_tok = self.unwrap_next_token()?;
        if let Token::Name(_, _, _, _name) = name_tok {
            let fields = self.parse_fields()?;

            let obj = Ok(ObjectTypeDefinitionNode::new(name_tok, description, fields)?);
            obj
        } else {
            Err(self.parse_error(String::from("Token::Name"), name_tok))
        }
    }

    fn parse_enum_type(&mut self, description: Description) -> ParseResult<EnumTypeDefinitionNode> {
        let name_tok = self.expect_token(Token::Name(0, 0, 0, "enum"))?;
        let values = self.parse_enum_values()?;
        Ok(EnumTypeDefinitionNode::new(name_tok, description, values)?)
    }

    fn parse_fields(&mut self) -> ParseResult<Vec<FieldDefinitionNode>> {
        let mut fields: Vec<FieldDefinitionNode> = Vec::new();
        self.expect_token(Token::OpenBrace(0, 0, 0))?;
        loop {
            if let Some(_) = self.expect_optional_token(&Token::CloseBrace(0, 0, 0)) {
                break;
            }
            fields.push(self.parse_field()?);
        }
        Ok(fields)
    }

    fn parse_field(&mut self) -> ParseResult<FieldDefinitionNode> {
        let description = self.parse_description()?;
        let name = self.expect_token(Token::Name(0,0,0,""))?;
        let arguments = self.parse_arguments()?;
        self.expect_token(Token::Colon(0,0,0))?;
        let field_type = self.parse_field_type()?;
        FieldDefinitionNode::new(name, field_type, description, arguments)
    }

    fn parse_field_type(&mut self) -> ParseResult<TypeNode> {
        let mut field_type: TypeNode;
        if let Some(_) = self.expect_optional_token(&Token::OpenSquare(0, 0, 0)) {
            field_type = TypeNode::List(
                ListTypeNode::new(self.parse_field_type()?)
            );
            self.expect_token(Token::CloseSquare(0,0,0))?;
        } else {
            field_type = TypeNode::Named(
                NamedTypeNode::new(
                    self.expect_token(Token::Name(0,0,0,""))?
                )?
            );
        }
        if let Some(_) = self.expect_optional_token(&Token::Bang(0,0,0)) {
            field_type = TypeNode::NonNull(
                Rc::new(field_type)
            );
        }
        Ok(field_type)
    }

    fn parse_enum_values(&mut self) -> ParseResult<Vec<EnumValueDefinitionNode>> {
        let mut values: Vec<EnumValueDefinitionNode> = Vec::new();
        self.expect_token(Token::OpenBrace(0, 0, 0))?;
        loop {
            if let Some(_) = self.expect_optional_token(&Token::CloseBrace(0, 0, 0)) {
                break;
            }
            let description = self.parse_description()?;
            let name = self.expect_token(Token::Name(0, 0, 0, ""))?;
            values.push(EnumValueDefinitionNode::new(name, description)?);
        }
        Ok(values)
    }

    fn parse_value(&mut self) -> ParseResult<Option<ValueNode>> {
        match self.expect_optional_token(&Token::Equals(0,0,0)) {
            Some(_) => {
                let tok = self.unwrap_peeked_token()?;
                match *tok {
                    Token::Name(_, _, _, value) => {
                        self.unwrap_next_token()?;
                        match value {
                            "true" => Ok(Some(ValueNode::Bool(BooleanValueNode { value: true }))),
                            "false" => Ok(Some(ValueNode::Bool(BooleanValueNode { value: false }))),
                            "null" => Ok(Some(ValueNode::Null)),
                            _ => Ok(Some(ValueNode::Enum(EnumValueNode { value: value.to_owned() })))
                        }
                    },
                    Token::Int(_, _, _, value) => {
                        self.unwrap_next_token()?;
                        Ok(Some(ValueNode::Int(IntValueNode { value })))
                    },
                    Token::Float(_, _, _, value) => {
                        self.unwrap_next_token()?;
                        Ok(Some(ValueNode::Float(FloatValueNode { value })))
                    },
                    Token::Str(_, _, _, _) | Token::BlockStr(_, _, _, _) => {
                        let str_tok = self.unwrap_next_token()?;
                        Ok(Some(ValueNode::Str(StringValueNode::new(str_tok)?)))
                    },
                    Token::Dollar(_, _, _) => {
                        // TODO Implement self.parse_variable
                        // Ok(self.parse_variable()?)
                        Ok(None)
                    },
                    Token::OpenSquare(_,_,_) => {
                        // TODO Implement self.parse_list()
                        // self.parse_list()?
                        Ok(None)
                    },
                    Token::OpenBrace(_, _, _) => {
                        // TODO Implement self.parse_object()
                        // self.parse_object()?
                        Ok(None)
                    }
                    _ => Ok(None)
                }
            },
            None => Ok(None)
        }
    }

    fn parse_error(&mut self, expected: String, received: Token) -> ParseError {
        ParseError::UnexpectedToken {
            expected,
            received: received.to_string().to_owned(),
        }
    }

    fn expect_token(&mut self, tok: Token<'i>) -> ParseResult<Token<'i>> {
        if let Some(next) = self.lexer.next() {
            match next {
                Ok(actual) => {
                    if actual.is_same_type(&tok) {
                        Ok(actual)
                    } else {
                        Err(ParseError::UnexpectedToken {
                            expected: tok.to_string(),
                            received: actual.to_string().to_owned(),
                        })
                    }
                },
                Err(e) => Err(ParseError::LexError(e)),
            }
        } else {
            Err(ParseError::EOF)
        }
    }

    fn expect_optional_token(&mut self, tok: &Token<'i>) -> Option<Token<'i>> {
        if let Some(next) = self.lexer.peek() {
            match next {
                Ok(actual) => {
                    if actual.is_same_type(tok) {
                        Some(self.lexer.next().unwrap().unwrap())
                    } else {
                        None
                    }
                },
                Err(_) => None
            }
        } else {
            None
        }
    }

    fn unwrap_peeked_token(&mut self) -> ParseResult<&Token<'i>> {
        match self.lexer.peek() {
            Some(res) => {
                match res {
                    Ok(tok) => {
                        Ok(tok)
                    },
                    Err(lex_error) => Err(ParseError::LexError(*lex_error)),
                }
            },
            None => Err(ParseError::EOF),
        }
    }

    fn unwrap_next_token(&mut self) -> ParseResult<Token<'i>> {
        match self.lexer.next() {
            Some(res) => {
                match res {
                    Ok(tok) => {
                        Ok(tok)
                    },
                    Err(lex_error) => Err(ParseError::LexError(lex_error)),
                }
            },
            None => Err(ParseError::EOF),
        }
    }
}

// struct Location<'a> {
//     start: Token<'a>,
//     end: Token<'a>,
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_constructs() {
        let ast = AST::new("test");
        assert!(ast.is_ok());
    }
}