use std::fs::File;
use std::io::Read;
use std::path::Path;

use gwtlib::Model;
use winnow::ascii::{line_ending, space0, space1, till_line_ending};
use winnow::combinator::{alt, cut_err, eof, preceded, repeat, terminated};
use winnow::error::{StrContext, AddContext, ContextError, ParserError};
use winnow::stream::Recoverable;
use winnow::{LocatingSlice, ModalParser, Parser};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum GWTParseError {
    #[error(transparent)]
    #[diagnostic(code(my_lib::io_error))]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    SyntaxError(#[from] SyntaxError),
}

#[derive(Error, Diagnostic, Debug)]
#[error("syntax error")]
pub struct SyntaxError {
    #[source_code]
    src: NamedSource<String>,
    #[label("me?")]
    pub at: SourceSpan,
}

// impl SyntaxError {
//     fn from_parse(error: ParseError<
// }

#[derive(Debug, PartialEq)]
pub enum GWTNode {
    Given(String),
    When(String),
    Then(String),
}

#[derive(Debug, PartialEq)]
struct Group {
    name: String,
    steps: Vec<Step>,
    indent: usize,
}
#[derive(Debug, PartialEq)]
struct Step {
    name: String,
    gwt_list: Vec<GWTNode>,
    indent: usize,
}

#[derive(Debug, PartialEq)]
enum GSNode {
    Group(Group),
    Step(Step),
}
#[derive(Debug, PartialEq)]
pub struct AST {
    working_on: String,
    nodes: Vec<GSNode>,
}

type Stream<'i> = Recoverable<LocatingSlice<&'i str>, ContextError>;

fn keyword<'i, F, O, E>(inner: F) -> impl ModalParser<Stream<'i>, (&'i str, usize), E>
where
    E: ParserError<Recoverable<LocatingSlice<&'i str>, ContextError>>
        + AddContext<Recoverable<LocatingSlice<&'i str>, ContextError>, StrContext>,
    F: ModalParser<Stream<'i>, O, E>,
{
    (
        space0,
        preceded(
            (inner, space1),
            terminated(
                till_line_ending,
                alt((eof, repeat(1.., line_ending).map(|()| ()).take())),
            ),
        ),
    )
        .map(|(indent, node): (&str, &str)| {
            let indent = indent.len();
            (node, indent)
        })
}
fn given<'i>(input: &mut Stream<'i>) -> winnow::ModalResult<(GWTNode, usize)> {
    keyword("GIVEN")
        .map(|(s, indent): (&'i str, usize)| (GWTNode::Given(s.into()), indent))
        .parse_next(input)
}

fn then<'i>(input: &mut Stream<'i>) -> winnow::ModalResult<(GWTNode, usize)> {
    keyword("THEN")
        .map(|(s, indent): (&'i str, usize)| (GWTNode::Then(s.into()), indent))
        .parse_next(input)
}
fn when<'i>(input: &mut Stream<'i>) -> winnow::ModalResult<(GWTNode, usize)> {
    keyword("WHEN")
        .map(|(s, indent): (&'i str, usize)| (GWTNode::When(s.into()), indent))
        .parse_next(input)
}

fn gwt(input: &mut Stream<'_>) -> winnow::ModalResult<(GWTNode, usize)> {
    alt((given, then, when,)).parse_next(input)
}

fn step(input: &mut Stream<'_>) -> winnow::ModalResult<Step> {
    let (step, step_indent) = keyword("STEP").parse_next(input)?;
    let (first_gwt, first_indent) = gwt
        .verify(|(_node, gwt_indent)| *gwt_indent > step_indent)
        .context(StrContext::Label("indent"))
        .parse_next(input)?;
    let rest_gwt: Vec<(GWTNode, usize)> = repeat(
        0..,
        gwt.verify(|(_node, gwt_indent)| *gwt_indent == first_indent)
            .context(StrContext::Label("indent")),  
    )
    .parse_next(input)?;
    let rest_gwt = rest_gwt.into_iter().map(|(node, _)| node);

    let mut gwt_list = vec![first_gwt];

    for node in rest_gwt {
        gwt_list.push(node);
    }

    Ok(Step {
        name: step.into(),
        indent: step_indent,
        gwt_list,
    })
}

fn group(input: &mut Stream<'_>) -> winnow::ModalResult<Group> {
    let (group, group_indent) = keyword("GROUP").parse_next(input)?;
    let first_step = cut_err(step)
        .verify(|step| step.indent > group_indent)
        .context(StrContext::Label("indent"))
        .parse_next(input)?;
    let rest_steps: Vec<Step> =
        repeat(0.., step.verify(|step| step.indent == first_step.indent)
            .context(StrContext::Label("indent")),  
            ).parse_next(input)?;
    let mut steps = vec![first_step];
    for node in rest_steps {
        steps.push(node);
    }
    Ok(Group {
        name: group.into(),
        indent: group_indent,
        steps,
    })
}

fn working_on(input: &mut Stream<'_>) -> winnow::ModalResult<String> {
    keyword("WORKING_ON")
        .verify_map(|(working, indent)| {
            if indent != 0 {
                return None;
            }
            Some(working.to_string())
        })
        .parse_next(input)
}

pub fn parse(input: &str) -> Result<AST, SyntaxError> {
    let input2 = Stream::new(LocatingSlice::new(input));
    (
        working_on,
        repeat(
            1..,
            alt((
                group.map(GSNode::Group),
                step.map(GSNode::Step),
            )),
        ),
    )
        .map(|(working_on, nodes)| AST { working_on, nodes })
        .parse(input2)
        .map_err(|e| {
            dbg!(&e);
            SyntaxError {
                src: NamedSource::new("test", input.to_string()),
                at: e.char_span().into(),
            }
        })
}

fn compress_step(step: Step) -> (Vec<String>, String, Vec<String>) {
    let given = Vec::new();
    let when = String::new();
    let then = Vec::new();
    step.gwt_list.into_iter().fold((given, when, then), |(mut given, mut when, mut then), next|{
        match next{
            GWTNode::When(w) => {
                when = when + &w;
            },
            GWTNode::Then(t) => {
                then.push(t);
            }
            GWTNode::Given(g) => {
                given.push(g);
            }
        }
        (given, when, then)
    })

}

pub fn parse_file(file_path: &Path) -> Result<Model, GWTParseError>{
    let mut file = File::open(file_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let ast = parse(&content)?;
    let mut model = Model::new();
    for gs_node in ast.nodes.into_iter() {
        match gs_node {
            GSNode::Step(step) => {
                let (given, when, then) = compress_step(step);
                model.add_step(&given, &when, &then);
            },
            GSNode::Group(g) => {
                for step in g.steps {
                    let (given, when, then) = compress_step(step);
                    model.add_step(&given, &when, &then);
                }
            },
        }
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_given() {
        let result = given(&mut Stream::new(LocatingSlice::new("GIVEN test 123"))).unwrap();
        assert_eq!(result, (GWTNode::Given("test 123".into()), 0))
    }
    #[test]
    fn test_then() {
        let result = then(&mut Stream::new(LocatingSlice::new("THEN test 123"))).unwrap();
        assert_eq!(result, (GWTNode::Then("test 123".into()), 0))
    }
    #[test]
    fn test_when() {
        let mut input = Stream::new(LocatingSlice::new(" WHEN test 123\n"));
        let result = when(&mut input).unwrap();
        assert_eq!(result, (GWTNode::When("test 123".into()), 1));
    }
    #[test]
    fn test_step() {
        let mut input = Stream::new(LocatingSlice::new(
            "STEP test 123
  WHEN LOL
  THEN Ha
",
        ));
        let result = step(&mut input).unwrap();
        dbg!(&input);
        assert_eq!(
            result,
            Step {
                name: "test 123".into(),
                indent: 0,
                gwt_list: vec![GWTNode::When("LOL".into()), GWTNode::Then("Ha".into()),]
            }
        )
    }
    #[test]
    fn test_group() {
        let mut input = Stream::new(LocatingSlice::new(
            "GROUP G1
  STEP test 123
    WHEN LOL
    THEN Ha
  STEP test 1234
    WHEN Test2
    THEN more tests
",
        ));
        let result = group(&mut input).unwrap();
        assert_eq!(
            result,
            Group {
                name: "G1".into(),
                indent: 0,
                steps: vec![
                    Step {
                        name: "test 123".into(),
                        indent: 2,
                        gwt_list: vec![GWTNode::When("LOL".into()), GWTNode::Then("Ha".into()),]
                    },
                    Step {
                        name: "test 1234".into(),
                        indent: 2,
                        gwt_list: vec![
                            GWTNode::When("Test2".into()),
                            GWTNode::Then("more tests".into()),
                        ]
                    }
                ]
            }
        )
    }
    #[test]
    fn test_working_on() {
        let result =
            working_on(&mut Stream::new(LocatingSlice::new("WORKING_ON test 123"))).unwrap();
        assert_eq!(result, "test 123".to_string())
    }
    #[test]
    fn test_parser() {
        let mut input = Stream::new(LocatingSlice::new(
            "WORKING_ON test
GROUP G1
    STEP test 123
        WHEN LOL
        THEN Ha
    STEP test 1234
        WHEN Test2
        THEN more tests
STEP one is the loneliest number
    WHEN LOL
    THEN Ha
GROUP G1
  STEP test 123
    WHEN LOL
    THEN Ha
  STEP test 1234
    WHEN Test2
    THEN more tests
",
        ));
        let result = parse(&mut input).unwrap();
        assert_eq!(
            result,
            AST {
                working_on: "test".to_string(),
                nodes: vec![
                    GSNode::Group(Group {
                        name: "G1".into(),
                        indent: 0,
                        steps: vec![
                            Step {
                                name: "test 123".into(),
                                indent: 4,
                                gwt_list: vec![
                                    GWTNode::When("LOL".into()),
                                    GWTNode::Then("Ha".into()),
                                ]
                            },
                            Step {
                                name: "test 1234".into(),
                                indent: 4,
                                gwt_list: vec![
                                    GWTNode::When("Test2".into()),
                                    GWTNode::Then("more tests".into()),
                                ]
                            }
                        ]
                    }),
                    GSNode::Step(Step {
                        name: "one is the loneliest number".into(),
                        indent: 0,
                        gwt_list: vec![GWTNode::When("LOL".into()), GWTNode::Then("Ha".into()),]
                    }),
                    GSNode::Group(Group {
                        name: "G1".into(),
                        indent: 0,
                        steps: vec![
                            Step {
                                name: "test 123".into(),
                                indent: 2,
                                gwt_list: vec![
                                    GWTNode::When("LOL".into()),
                                    GWTNode::Then("Ha".into()),
                                ]
                            },
                            Step {
                                name: "test 1234".into(),
                                indent: 2,
                                gwt_list: vec![
                                    GWTNode::When("Test2".into()),
                                    GWTNode::Then("more tests".into()),
                                ]
                            }
                        ]
                    }),
                ]
            }
        )
    }
}
