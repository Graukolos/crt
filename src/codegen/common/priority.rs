use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::ast::{Action, Actor};

pub struct Priorities {
    edges: BTreeSet<(String, String)>,
}

#[derive(Default)]
pub struct Hold {
    pub own: Option<String>,
    pub blocked_by: Vec<String>,
}

impl Priorities {
    pub fn new(actor: &Actor) -> Self {
        let mut edges: BTreeSet<(String, String)> = BTreeSet::new();
        for chain in &actor.priorities {
            let resolved: Vec<Vec<&str>> = chain
                .order
                .iter()
                .map(|entry| {
                    let names = resolve(actor, entry);
                    if names.is_empty() {
                        eprintln!(
                            "warning: actor {}: priority entry '{entry}' matches no action; ignoring it",
                            actor.name
                        );
                    }
                    names
                })
                .collect();

            for (rank, highs) in resolved.iter().enumerate() {
                for lows in &resolved[rank + 1..] {
                    for high in highs {
                        for low in lows {
                            if high != low {
                                edges.insert(((*high).to_string(), (*low).to_string()));
                            }
                        }
                    }
                }
            }
        }
        Self {
            edges: transitive_closure(edges),
        }
    }

    fn outranks(&self, high: &str, low: &str) -> bool {
        self.edges.contains(&(high.to_string(), low.to_string()))
    }

    pub fn order(&self, actor: &Actor, candidates: &[&Action]) -> Vec<usize> {
        if self.edges.is_empty() {
            return (0..candidates.len()).collect();
        }

        let mut remaining: Vec<usize> = (0..candidates.len()).collect();
        let mut ordered = Vec::with_capacity(candidates.len());
        while !remaining.is_empty() {
            let ready = remaining.iter().position(|&i| {
                !remaining
                    .iter()
                    .any(|&j| j != i && self.outranks(&candidates[j].name, &candidates[i].name))
            });
            if let Some(pos) = ready {
                ordered.push(remaining.remove(pos));
            } else {
                let stuck: Vec<&str> = remaining
                    .iter()
                    .map(|&i| candidates[i].name.as_str())
                    .collect();
                eprintln!(
                    "warning: actor {}: priorities are cyclic over [{}]; using declaration order for those actions",
                    actor.name,
                    stuck.join(", ")
                );
                ordered.append(&mut remaining);
            }
        }
        ordered
    }

    pub fn holds(&self, ordered: &[&Action]) -> (String, Vec<Hold>) {
        let flag = |pos: usize| format!("__held_{pos}");
        let mut decls = String::new();
        let mut holds = Vec::with_capacity(ordered.len());
        for (pos, action) in ordered.iter().enumerate() {
            let own = ordered[pos + 1..]
                .iter()
                .any(|low| self.outranks(&action.name, &low.name))
                .then(|| flag(pos));
            if let Some(own) = &own {
                let _ = writeln!(decls, "            let mut {own} = false;");
            }
            let blocked_by = ordered[..pos]
                .iter()
                .enumerate()
                .filter(|(_, high)| self.outranks(&high.name, &action.name))
                .map(|(i, _)| flag(i))
                .collect();
            holds.push(Hold { own, blocked_by });
        }
        (decls, holds)
    }
}

fn transitive_closure(mut edges: BTreeSet<(String, String)>) -> BTreeSet<(String, String)> {
    loop {
        let implied: Vec<(String, String)> = edges
            .iter()
            .flat_map(|(a, b)| {
                edges
                    .iter()
                    .filter(move |(c, d)| c == b && d != a)
                    .map(move |(_, d)| (a.clone(), d.clone()))
            })
            .filter(|edge| !edges.contains(edge))
            .collect();
        if implied.is_empty() {
            return edges;
        }
        edges.extend(implied);
    }
}

fn resolve<'a>(actor: &'a Actor, entry: &str) -> Vec<&'a str> {
    actor
        .actions
        .iter()
        .map(|action| action.name.as_str())
        .filter(|name| !name.is_empty() && tag_matches(name, entry))
        .collect()
}

fn tag_matches(name: &str, entry: &str) -> bool {
    name == entry || (name.starts_with(entry) && name.as_bytes().get(entry.len()) == Some(&b'.'))
}
