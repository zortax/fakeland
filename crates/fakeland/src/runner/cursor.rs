//! The step cursor of a run. It walks the steps in order and enters each repeat block for its
//! count of iterations. Each step carries its path, such as `steps[1].steps[0]`.

use std::rc::Rc;

use fakeland_scenario::Step;

/// The next step to run.
#[derive(Clone, Debug, PartialEq)]
pub struct NextStep {
    /// The step.
    pub step: Step,
    /// The path of the step in the scenario.
    pub path: String,
    /// The iteration of the innermost repeat block, for steps inside one.
    pub iteration: Option<u32>,
}

#[derive(Debug, Clone)]
struct Frame {
    steps: Rc<[Step]>,
    next: usize,
    iteration: u32,
    count: u32,
    path: String,
    nested: bool,
}

/// Walks the steps of a scenario.
#[derive(Debug, Clone)]
pub struct Cursor {
    frames: Vec<Frame>,
}

impl Cursor {
    /// Starts at the first step.
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            frames: vec![Frame {
                steps: steps.into(),
                next: 0,
                iteration: 0,
                count: 1,
                path: "steps".into(),
                nested: false,
            }],
        }
    }

    /// Returns the next step, or `None` after the last one.
    pub fn next_step(&mut self) -> Option<NextStep> {
        loop {
            let top = self.frames.last_mut()?;
            if top.next < top.steps.len() {
                let i = top.next;
                top.next += 1;
                return Some(NextStep {
                    step: top.steps[i].clone(),
                    path: format!("{}[{i}]", top.path),
                    iteration: top.nested.then_some(top.iteration),
                });
            }
            if top.iteration + 1 < top.count {
                top.iteration += 1;
                top.next = 0;
                continue;
            }
            if self.frames.len() == 1 {
                return None;
            }
            self.frames.pop();
        }
    }

    /// Enters a repeat block of the step at `path`.
    pub fn enter_repeat(&mut self, steps: &[Step], count: u32, path: &str) {
        if count == 0 || steps.is_empty() {
            return;
        }
        self.frames.push(Frame {
            steps: steps.to_vec().into(),
            next: 0,
            iteration: 0,
            count,
            path: format!("{path}.steps"),
            nested: true,
        });
    }
}

#[cfg(test)]
mod tests {
    use fakeland_scenario::Action;

    use super::*;

    fn mark(l: &str) -> Step {
        Step::new(Action::mark(l))
    }

    #[test]
    fn walks_steps_and_repeats() {
        let inner = vec![mark("a"), mark("b")];
        let mut c = Cursor::new(vec![
            mark("start"),
            Step::new(Action::Repeat {
                count: 2,
                steps: inner.clone(),
            }),
            mark("end"),
        ]);
        let mut seen = Vec::new();
        while let Some(n) = c.next_step() {
            if let Action::Repeat { count, steps } = &n.step.action {
                c.enter_repeat(steps, *count, &n.path);
            }
            seen.push((n.path, n.iteration));
        }
        assert_eq!(
            seen,
            vec![
                ("steps[0]".to_string(), None),
                ("steps[1]".to_string(), None),
                ("steps[1].steps[0]".to_string(), Some(0)),
                ("steps[1].steps[1]".to_string(), Some(0)),
                ("steps[1].steps[0]".to_string(), Some(1)),
                ("steps[1].steps[1]".to_string(), Some(1)),
                ("steps[2]".to_string(), None),
            ]
        );
    }

    #[test]
    fn empty_and_zero_repeats_are_skipped() {
        let mut c = Cursor::new(vec![
            Step::new(Action::Repeat {
                count: 0,
                steps: vec![mark("x")],
            }),
            mark("y"),
        ]);
        let first = c.next_step().unwrap();
        if let Action::Repeat { count, steps } = &first.step.action {
            c.enter_repeat(steps, *count, &first.path);
        }
        assert_eq!(c.next_step().unwrap().path, "steps[1]");
        assert_eq!(c.next_step(), None);
        assert_eq!(c.next_step(), None);
    }
}
