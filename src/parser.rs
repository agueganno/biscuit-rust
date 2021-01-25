//! Datalog text format parsing
//!
//! all of the parsers are usable with [`TryFrom`] so they can be used
//! as follows:
//!
//! ```rust
//! use std::convert::TryInto;
//! use biscuit_auth::token::builder::Fact;
//!
//! let f: Fact = "test(#data)".try_into().expect("parse error");
//! ```
//!
//! All of the methods in [BiscuitBuilder](`crate::token::builder::BiscuitBuilder`)
//! and [BlockBuilder](`crate::token::builder::BlockBuilder`) can take strings
//! as arguments too
use crate::{error, token::builder};
use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, tag, tag_no_case, take_while1},
    character::{
        complete::{char, digit1, multispace0 as space0},
        is_alphanumeric,
    },
    combinator::{map, map_res, opt, recognize, value},
    multi::{separated_list0, separated_list1, many0},
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};
use std::{
    convert::{TryFrom, TryInto},
    str::FromStr,
    collections::BTreeSet,
};

/// parse a Datalog fact
pub fn fact(i: &str) -> IResult<&str, builder::Fact> {
    predicate(i).map(|(i, p)| (i, builder::Fact(p)))
}

/// parse a Datalog caveat
pub fn caveat(i: &str) -> IResult<&str, builder::Caveat> {
    let (i, _) = space0(i)?;

    let (i, _) = tag_no_case("check if")(i)?;

    let (i, queries) = caveat_body(i)?;
    Ok((i, builder::Caveat { queries }))
}

/// parse an allow rule
pub fn policy(i: &str) -> IResult<&str, builder::Policy> {
    alt((allow, deny))(i)
}

/// parse an allow rule
pub fn allow(i: &str) -> IResult<&str, builder::Policy> {
    let (i, _) = space0(i)?;

    let (i, _) = tag_no_case("allow if")(i)?;

    let (i, queries) = caveat_body(i)?;
    Ok((i, builder::Policy { queries, kind: builder::PolicyKind::Allow }))
}

/// parse an allow rule
pub fn deny(i: &str) -> IResult<&str, builder::Policy> {
    let (i, _) = space0(i)?;

    let (i, _) = tag_no_case("deny if")(i)?;

    let (i, queries) = caveat_body(i)?;
    Ok((i, builder::Policy { queries, kind: builder::PolicyKind::Deny }))
}

/// parse a Datalog caveat body
pub fn caveat_body(i: &str) -> IResult<&str, Vec<builder::Rule>> {
    let (i, mut queries) = separated_list1(
      preceded(space0, tag_no_case("or")),
      preceded(space0, rule_body)
    )(i)?;

    let queries = queries.drain(..).map(|rule_body| {
        builder::Rule(
            builder::Predicate {
                name: "query".to_string(),
                ids: Vec::new(),
            },
            rule_body.0,
            rule_body.1
        )
    }).collect();
    Ok((i, queries))
}

/// parse a Datalog rule
pub fn rule(i: &str) -> IResult<&str, builder::Rule> {
    let (i, head) = rule_head(i)?;
    let (i, _) = space0(i)?;

    let (i, _) = tag("<-")(i)?;

    let (i, (predicates, expressions)) = rule_body(i)?;

    Ok((i, builder::Rule(head, predicates, expressions)))
}

impl TryFrom<&str> for builder::Fact {
    type Error = error::Token;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        fact(value)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl TryFrom<&str> for builder::Rule {
    type Error = error::Token;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        rule(value)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl FromStr for builder::Fact {
    type Err = error::Token;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fact(s)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl FromStr for builder::Rule {
    type Err = error::Token;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        rule(s)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl TryFrom<&str> for builder::Caveat {
    type Error = error::Token;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        caveat(value)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl FromStr for builder::Caveat {
    type Err = error::Token;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        caveat(s)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl TryFrom<&str> for builder::Policy {
    type Error = error::Token;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        policy(value)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl FromStr for builder::Policy {
    type Err = error::Token;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        policy(s)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

impl FromStr for builder::Predicate {
    type Err = error::Token;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        predicate(s)
            .map(|(_, o)| o)
            .map_err(|_| error::Token::ParseError)
    }
}

fn predicate(i: &str) -> IResult<&str, builder::Predicate> {
    let (i, _) = space0(i)?;
    let (i, fact_name) = name(i)?;

    let (i, _) = space0(i)?;
    let (i, ids) = delimited(
        char('('),
        separated_list1(preceded(space0, char(',')), term),
        preceded(space0, char(')')),
    )(i)?;

    Ok((
        i,
        builder::Predicate {
            name: fact_name.to_string(),
            ids,
        },
    ))
}

fn rule_head(i: &str) -> IResult<&str, builder::Predicate> {
    let (i, _) = space0(i)?;
    let (i, fact_name) = name(i)?;

    let (i, _) = space0(i)?;
    let (i, ids) = delimited(
        char('('),
        separated_list0(preceded(space0, char(',')), term),
        preceded(space0, char(')')),
    )(i)?;

    Ok((
        i,
        builder::Predicate {
            name: fact_name.to_string(),
            ids,
        },
    ))
}

/// parse a Datalog rule body
pub fn rule_body(i: &str) -> IResult<&str, (Vec<builder::Predicate>, Vec<builder::Expression>)> {

    let (i, mut elements) = separated_list1(
      preceded(space0, char(',')),
      preceded(space0, predicate_or_expression)
    )(i)?;

    let mut predicates = Vec::new();
    let mut expressions = Vec::new();

    for el in elements.drain(..) {
        match el {
            PredOrExpr::P(predicate) => predicates.push(predicate),
            PredOrExpr::E(expression) => {
                let ops = expression.opcodes();
                let e = builder::Expression { ops };
                expressions.push(e);
            },
        }
    }

    Ok((i, (predicates, expressions)))
}

enum PredOrExpr {
  P(builder::Predicate),
  E(Expr),
}

fn predicate_or_expression(i: &str) -> IResult<&str, PredOrExpr> {
    alt((
        map(predicate, PredOrExpr::P),
        map(expr, PredOrExpr::E),
    ))(i)
}


#[derive(Debug, PartialEq)]
pub enum Expr {
  Value(builder::Term),
  Unary(builder::Op, Box<Expr>),
  Binary(builder::Op, Box<Expr>, Box<Expr>),
}

impl Expr {
    pub fn opcodes(self) -> Vec<builder::Op> {
        let mut v = Vec::new();
        self.into_opcodes(&mut v);
        v
    }

    fn into_opcodes(self, v: &mut Vec<builder::Op>) {
        match self {
            Expr::Value(t) => v.push(builder::Op::Value(t)),
            Expr::Unary(op, expr) => {
                expr.into_opcodes(v);
                v.push(op);
            },
            Expr::Binary(op, left, right) => {
                left.into_opcodes(v);
                right.into_opcodes(v);
                v.push(op);
            },

        }
    }
}

fn unary(i: &str) -> IResult<&str, Expr> {
    let (i, _) = space0(i)?;
    let (i, _) = tag("-")(i)?;
    let (i, _) = space0(i)?;
    let (i, value) = expr(i)?;

    Ok((i, Expr::Unary(builder::Op::Unary(builder::Unary::Negate), Box::new(value))))
}

fn binary_op_0(i: &str) -> IResult<&str, builder::Binary> {
    use builder::Binary;
    value(Binary::And, tag("&&"))(i)
}

fn binary_op_1(i: &str) -> IResult<&str, builder::Binary> {
    use builder::Binary;
    alt((
        value(Binary::LessOrEqual, tag("<=")),
        value(Binary::GreaterOrEqual, tag(">=")),
        value(Binary::LessThan, tag("<")),
        value(Binary::GreaterThan, tag(">")),
        value(Binary::Equal, tag("==")),
    ))(i)
}

fn binary_op_2(i: &str) -> IResult<&str, builder::Binary> {
    use builder::Binary;
    value(Binary::Add, tag("+"))(i)
}

fn binary_op_3(i: &str) -> IResult<&str, builder::Binary> {
    use builder::Binary;
    alt((
        value(Binary::In, tag("in")),
        value(Binary::NotIn, tag("not in")),
        value(Binary::Prefix, tag("starts with")),
        value(Binary::Suffix, tag("ends with")),
        value(Binary::Regex, tag("matches")),
    ))(i)
}

fn expr_term(i: &str) -> IResult<&str, Expr> {
    alt((
        unary,
        map(term, Expr::Value),
    ))(i)
}

fn fold_exprs(initial: Expr, remainder: Vec<(builder::Binary, Expr)>) -> Expr {
  remainder.into_iter().fold(initial, |acc, pair| {
    let (op, expr) = pair;
    Expr::Binary(builder::Op::Binary(op), Box::new(acc), Box::new(expr))
  })
}

fn expr(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = expr1(i)?;

    let (i, remainder) = many0(tuple((preceded(space0, binary_op_0), expr1)))(i)?;

    Ok((i, fold_exprs(initial, remainder)))
}

fn expr1(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = expr2(i)?;

    let (i, remainder) = many0(tuple((preceded(space0, binary_op_1), expr2)))(i)?;

    Ok((i, fold_exprs(initial, remainder)))
}

fn expr2(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = expr3(i)?;

    let (i, remainder) = many0(tuple((preceded(space0, binary_op_2), expr3)))(i)?;

    Ok((i, fold_exprs(initial, remainder)))
}

fn expr3(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = expr_term(i)?;

    let (i, remainder) = many0(tuple((preceded(space0, binary_op_3), expr_term)))(i)?;

    Ok((i, fold_exprs(initial, remainder)))
}

fn name(i: &str) -> IResult<&str, &str> {
    let is_name_char = |c: char| is_alphanumeric(c as u8) || c == '_';

    take_while1(is_name_char)(i)
}

fn printable(i: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c != '\\' && c != '"')(i)
}

fn parse_string_internal(i: &str) -> IResult<&str, String> {
    escaped_transform(
        printable,
        '\\',
        alt((
            map(char('\\'), |_| "\\"),
            map(char('"'), |_| "\""),
            map(char('n'), |_| "\n"),
        )),
    )(i)
}

fn parse_string(i: &str) -> IResult<&str, String> {
    delimited(char('"'), parse_string_internal, char('"'))(i)
}

fn string(i: &str) -> IResult<&str, builder::Term> {
    parse_string(i).map(|(i, s)| (i, builder::Term::Str(s)))
}

fn parse_symbol(i: &str) -> IResult<&str, &str> {
    preceded(char('#'), name)(i)
}

fn symbol(i: &str) -> IResult<&str, builder::Term> {
    parse_symbol(i).map(|(i, s)| (i, builder::s(s)))
}

fn parse_integer(i: &str) -> IResult<&str, i64> {
    map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| s.parse())(i)
}

fn integer(i: &str) -> IResult<&str, builder::Term> {
    parse_integer(i).map(|(i, n)| (i, builder::int(n)))
}

fn parse_date(i: &str) -> IResult<&str, u64> {
    map_res(
        map_res(take_while1(|c: char| c != ',' && c != ' ' && c != ')'), |s| {
            let r = chrono::DateTime::parse_from_rfc3339(s);
            r
        }),
        |t| {
            let r = t.timestamp().try_into();
            r
        },
    )(i)
}

fn date(i: &str) -> IResult<&str, builder::Term> {
    parse_date(i).map(|(i, t)| (i, builder::Term::Date(t)))
}

fn parse_bytes(i: &str) -> IResult<&str, Vec<u8>> {
    preceded(
        tag("hex:"),
        map_res(
            take_while1(|c| {
                let c = c as u8;
                (b'0' <= c && c <= b'9')
                    || (b'a' <= c && c <= b'f')
                    || (b'A' <= c && c <= b'F')
            }),
            hex::decode
        )
    )(i)
}

fn bytes(i: &str) -> IResult<&str, builder::Term> {
    parse_bytes(i).map(|(i, s)| (i, builder::Term::Bytes(s)))
}

fn variable(i: &str) -> IResult<&str, builder::Term> {
    map(
        preceded(char('$'), name),
        builder::variable,
    )(i)
}

fn parse_bool(i: &str) -> IResult<&str, bool> {
    alt((
        value(true, tag("true")),
        value(false, tag("false")),
    ))(i)
}

fn boolean(i: &str) -> IResult<&str, builder::Term> {
    parse_bool(i).map(|(i, b)| (i, builder::boolean(b)))
}

//FIXME: replace panics with proper parse errors
fn set(i: &str) -> IResult<&str, builder::Term> {
    //println!("set:\t{}", i);
    let (i, _) = preceded(space0, char('['))(i)?;
    let (i, mut list) = separated_list1(preceded(space0, char(',')), term_in_set)(i)?;

    let mut set = BTreeSet::new();

    let mut kind: Option<u8> = None;
    for term in list.drain(..) {
        let index = match term {
            builder::Term::Symbol(_) => 0,
            builder::Term::Variable(_) => panic!("variables are not permitted in sets"),
            builder::Term::Integer(_) => 2,
            builder::Term::Str(_) => 3,
            builder::Term::Date(_) => 4,
            builder::Term::Bytes(_) => 5,
            builder::Term::Bool(_) => 6,
            builder::Term::Set(_) => panic!("sets cannot contain other sets"),
        };

        if let Some(k) = kind {
            if k != index {
                panic!("set elements must have the same type");
            }
        } else {
            kind = Some(index);
        }

        set.insert(term);
    }

    let (i, _) = preceded(space0, char(']'))(i)?;

    Ok((i, builder::set(set)))
}

fn term(i: &str) -> IResult<&str, builder::Term> {
    preceded(space0, alt((symbol, string, date, variable, integer, bytes, boolean, set)))(i)
}

fn term_in_set(i: &str) -> IResult<&str, builder::Term> {
    preceded(space0, alt((symbol, string, date, integer, bytes, boolean)))(i)
}

fn regex(i: &str) -> IResult<&str, String> {
    delimited(
        char('/'),
        escaped_transform(
            take_while1(|c: char| c != '\\' && c != '/'),
            '\\',
            alt((
                map(char('\\'), |_| "\\"),
                map(char('"'), |_| "\""),
                map(char('n'), |_| "\n"),
                map(char('/'), |_| "/"),
            )),
        ),
        char('/'),
    )(i)
}

#[cfg(test)]
mod tests {
    use crate::{datalog, token::builder};
    use std::collections::HashSet;

    #[test]
    fn name() {
        assert_eq!(
            super::name("operation(#ambient, #read)"),
            Ok(("(#ambient, #read)", "operation"))
        );
    }

    #[test]
    fn symbol() {
        assert_eq!(super::symbol("#ambient"), Ok(("", builder::s("ambient"))));
    }

    #[test]
    fn string() {
        assert_eq!(
            super::string("\"file1 a hello - 123_\""),
            Ok(("", builder::string("file1 a hello - 123_")))
        );
    }

    #[test]
    fn integer() {
        assert_eq!(super::integer("123"), Ok(("", builder::int(123))));
        assert_eq!(super::integer("-42"), Ok(("", builder::int(-42))));
    }

    #[test]
    fn date() {
        assert_eq!(
            super::date("2019-12-02T13:49:53Z"),
            Ok(("", builder::Term::Date(1575294593)))
        );
    }

    #[test]
    fn variable() {
        assert_eq!(super::variable("$1"), Ok(("", builder::variable("1"))));
    }

    #[test]
    fn constraint() {
        use builder::{Expression, Op, Binary, Unary, date, var, int, set, string, symbol};
        use std::time::{SystemTime, Duration};
        use std::collections::BTreeSet;

        assert_eq!(
            super::expr("$0 <= 2030-12-31T12:59:59+00:00")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(date(&(SystemTime::UNIX_EPOCH + Duration::from_secs(1924952399)))),
                    Op::Binary(Binary::LessOrEqual),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 >= 2030-12-31T12:59:59+00:00")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(date(&(SystemTime::UNIX_EPOCH + Duration::from_secs(1924952399)))),
                    Op::Binary(Binary::GreaterOrEqual),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 < 1234")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(int(1234)),
                    Op::Binary(Binary::LessThan),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 > 1234")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(int(1234)),
                    Op::Binary(Binary::GreaterThan),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 <= 1234")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(int(1234)),
                    Op::Binary(Binary::LessOrEqual),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 >= -1234")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(int(1234)),
                    Op::Unary(Unary::Negate),
                    Op::Binary(Binary::GreaterOrEqual),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 == 1")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(int(1)),
                    Op::Binary(Binary::Equal),
                ],
            ))
        );

        let h = [int(1), int(2)].iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(
            super::expr("$0 in [1, 2]")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(set(h.clone())),
                    Op::Binary(Binary::In),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 not in [1, 2]")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(set(h)),
                    Op::Binary(Binary::NotIn),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 == \"abc\"")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(string("abc")),
                    Op::Binary(Binary::Equal),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 ends with \"abc\"")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(string("abc")),
                    Op::Binary(Binary::Suffix),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 starts with \"abc\"")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(string("abc")),
                    Op::Binary(Binary::Prefix),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 matches \"abc[0-9]+\"")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(string("abc[0-9]+")),
                    Op::Binary(Binary::Regex),
                ],
            ))
        );

        let h = [string("abc"), string("def")]
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            super::expr("$0 in [\"abc\", \"def\"]")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(set(h.clone())),
                    Op::Binary(Binary::In),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 not in [\"abc\", \"def\"]")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(set(h.clone())),
                    Op::Binary(Binary::NotIn),
                ],
            ))
        );

        let h = [symbol("abc"), symbol("def")]
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            super::expr("$0 in [#abc, #def]")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(set(h.clone())),
                    Op::Binary(Binary::In),
                ],
            ))
        );

        assert_eq!(
            super::expr("$0 not in [#abc, #def]")
                .map(|(i, o)| (i, o.opcodes())),
            Ok((
                "",
                vec![
                    Op::Value(var("0")),
                    Op::Value(set(h.clone())),
                    Op::Binary(Binary::NotIn),
                ],
            ))
        );
    }

    #[test]
    fn fact() {
        assert_eq!(
            super::fact("right( #authority, \"file1\", #read )"),
            Ok((
                "",
                builder::fact(
                    "right",
                    &[
                        builder::s("authority"),
                        builder::string("file1"),
                        builder::s("read")
                    ]
                )
            ))
        );
    }

    #[test]
    fn fact_with_date() {
        assert_eq!(
            super::fact("date(#ambient,2019-12-02T13:49:53Z)"),
            Ok(("",
                builder::fact(
                    "date",
                    &[
                        builder::s("ambient"),
                        builder::Term::Date(1575294593)
                    ]
                )
            ))
        );
    }


    #[test]
    fn rule() {
        assert_eq!(
            super::rule("right(#authority, $0, #read) <- resource( #ambient, $0), operation(#ambient, #read)"),
            Ok((
                "",
                builder::rule(
                    "right",
                    &[
                        builder::s("authority"),
                        builder::variable("0"),
                        builder::s("read"),
                    ],
                    &[
                        builder::pred("resource", &[builder::s("ambient"), builder::variable("0")]),
                        builder::pred("operation", &[builder::s("ambient"), builder::s("read")]),
                    ]
                )
            ))
        );
    }

    #[test]
    fn constrained_rule() {
        use builder::{Expression, Op, Binary, var, date};
        use std::time::{SystemTime, Duration};

        assert_eq!(
            super::rule("valid_date(\"file1\") <- time(#ambient, $0 ), resource( #ambient, \"file1\"), $0 <= 2019-12-04T09:46:41+00:00"),
            Ok((
                "",
                builder::constrained_rule(
                    "valid_date",
                    &[
                        builder::string("file1"),
                    ],
                    &[
                        builder::pred("time", &[builder::s("ambient"), builder::variable("0")]),
                        builder::pred("resource", &[builder::s("ambient"), builder::string("file1")]),
                    ],
                    &[Expression {
                        ops: vec![
                            Op::Value(var("0")),
                            Op::Value(date(&(SystemTime::UNIX_EPOCH + Duration::from_secs(1575452801)))),
                            Op::Binary(Binary::LessOrEqual),
                        ]
                    }],
                )
            ))
        );
    }

    #[test]
    fn constrained_rule_ordering() {
        use builder::{Expression, Op, Binary, var, date};
        use std::time::{SystemTime, Duration};

        assert_eq!(
            super::rule("valid_date(\"file1\") <- time(#ambient, $0 ), $0 <= 2019-12-04T09:46:41+00:00, resource(#ambient, \"file1\")"),
            Ok((
                "",
                builder::constrained_rule(
                    "valid_date",
                    &[
                        builder::string("file1"),
                    ],
                    &[
                        builder::pred("time", &[builder::s("ambient"), builder::variable("0")]),
                        builder::pred("resource", &[builder::s("ambient"), builder::string("file1")]),
                    ],
                    &[Expression {
                        ops: vec![
                            Op::Value(var("0")),
                            Op::Value(date(&(SystemTime::UNIX_EPOCH + Duration::from_secs(1575452801)))),
                            Op::Binary(Binary::LessOrEqual),
                        ]
                    }],
                )
            ))
        );
    }
    #[test]
    fn caveat() {
        let empty: &[builder::Term] = &[];
        assert_eq!(
            super::caveat("check if resource(#ambient, $0), operation(#ambient, #read) or admin(#authority)"),
            Ok((
                "",
                builder::Caveat {
                    queries: vec![
                        builder::rule(
                            "query",
                            empty,
                            &[
                                builder::pred("resource", &[builder::s("ambient"), builder::variable("0")]),
                                builder::pred("operation", &[builder::s("ambient"), builder::s("read")]),
                            ]
                        ),
                        builder::rule(
                            "query",
                            empty,
                            &[
                                builder::pred("admin", &[builder::s("authority")]),
                            ]
                        ),
                    ]
                }
            ))
        );
    }

    #[test]
    fn expression() {
        use builder::{Op, Unary, Binary, Term, var, date, int, string};
        use super::Expr;
        use std::time::{SystemTime, Duration};
        use crate::datalog::SymbolTable;

        let mut syms = SymbolTable::new();

        let input = " - 1 ";
        println!("parsing: {}", input);
        let res = super::expr(input);
        assert_eq!(
            res,
            Ok((
                " ",
                Expr::Unary(Op::Unary(Unary::Negate), Box::new(Expr::Value(Term::Integer(1))))
            ))
        );

        let ops = res.unwrap().1.opcodes();
        println!("ops: {:#?}", ops);
        let e = builder::Expression { ops }.convert(&mut syms);
        println!("print: {}", e.print(&syms).unwrap());

        let input = " $0 <= 2019-12-04T09:46:41+00:00";
        println!("parsing: {}", input);
        let res = super::expr(input);
        assert_eq!(
            res,
            Ok((
                "",
                Expr::Binary(
                    Op::Binary(Binary::LessOrEqual),
                    Box::new(Expr::Value(var("0"))),
                    Box::new(Expr::Value(date(&(SystemTime::UNIX_EPOCH + Duration::from_secs(1575452801))))
                ))
            ))
        );

        let ops = res.unwrap().1.opcodes();
        println!("ops: {:#?}", ops);
        let e = builder::Expression { ops }.convert(&mut syms);
        println!("print: {}", e.print(&syms).unwrap());

        let input = " 1 < $test + 2 ";
        println!("parsing: {}", input);
        let res = super::expr(input);
        assert_eq!(
            res,
            Ok((
                " ",
                Expr::Binary(
                    Op::Binary(Binary::LessThan),
                    Box::new(Expr::Value(int(1))),
                    Box::new(Expr::Binary(
                        Op::Binary(Binary::Add),
                        Box::new(Expr::Value(var("test"))),
                        Box::new(Expr::Value(int(2))),
                    ))
                )
            ))
        );

        let ops = res.unwrap().1.opcodes();
        println!("ops: {:#?}", ops);
        let e = builder::Expression { ops }.convert(&mut syms);
        println!("print: {}", e.print(&syms).unwrap());

        let input = " 2 < $test && $var2 starts with \"test\" && true ";
        println!("parsing: {}", input);
        let res = super::expr(input);
        assert_eq!(
            res,
            Ok((
                " ",
                Expr::Binary(
                    Op::Binary(Binary::And),
                    Box::new(Expr::Binary(
                        Op::Binary(Binary::And),
                        Box::new(Expr::Binary(
                            Op::Binary(Binary::LessThan),
                            Box::new(Expr::Value(int(2))),
                            Box::new(Expr::Value(var("test"))),
                        )),
                        Box::new(Expr::Binary(
                            Op::Binary(Binary::Prefix),
                            Box::new(Expr::Value(var("var2"))),
                            Box::new(Expr::Value(string("test"))),
                        )),
                    )),
                Box::new(Expr::Value(Term::Bool(true))),
    )
            ))
        );

        let ops = res.unwrap().1.opcodes();
        println!("ops: {:#?}", ops);
        let e = builder::Expression { ops }.convert(&mut syms);
        println!("print: {}", e.print(&syms).unwrap());

        //panic!();
    }
}
