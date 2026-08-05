use crate::ast::{Action, Actor};

pub struct Priorities {
    edges: Vec<(String, String)>,
}

impl Priorities {
    pub fn new(actor: &Actor) -> Self {
        let mut edges: Vec<(String, String)> = Vec::new();
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
                            if high == low {
                                continue;
                            }
                            let pair = ((*high).to_string(), (*low).to_string());
                            if !edges.contains(&pair) {
                                edges.push(pair);
                            }
                        }
                    }
                }
            }
        }
        Self { edges }
    }

    fn outranks(&self, high: &str, low: &str) -> bool {
        self.edges
            .iter()
            .any(|(h, l)| h.as_str() == high && l.as_str() == low)
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
