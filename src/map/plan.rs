use crate::map::{Edge, Edges, Node, Nodes};
use std::cmp::{Ord, Ordering};
use std::collections::{BinaryHeap, HashMap, HashSet};

pub struct Planner<'a, N, E> {
    nodes: &'a Nodes<N>,
    edges: &'a Edges<E>,
}

impl<'a, N, E> Planner<'a, N, E>
where
    N: Node,
    E: Edge,
{
    pub fn new(nodes: &'a Nodes<N>, edges: &'a Edges<E>) -> Self {
        Self { nodes, edges }
    }

    /// 使用bfs进行路径搜索，返回的行走计划为路径栈。
    /// 出栈过程即顺序行走
    pub fn walk(&self, fromid: u32, toid: u32) -> Vec<&E> {
        if !self.nodes.contains(fromid) || !self.nodes.contains(toid) {
            return vec![];
        }
        // 这里使用Rc和RefCell是为了让优先队列与哈希表共用相同
        let mut candidates = BinaryHeap::<Weight<E>>::new();
        let mut prev = HashMap::<u32, Weight<E>>::new();
        let mut reached = std::u32::MAX;

        let pseudo_path = E::pseudo(fromid);
        candidates.push(Weight {
            weight: 0,
            edge: &pseudo_path,
        });
        while let Some(curr) = candidates.pop() {
            if curr.weight >= reached {
                // 最小权重大于等于已到达权重，当前计划为最优计划
                break;
            }
            for e in self.edges.exits(curr.edge.endid()) {
                let curr_weight = curr.weight + e.weight();
                if curr_weight < reached {
                    // 当前权重小于可到达
                    if let Some(cal) = prev.get(&e.endid()) {
                        // 下一个房间曾经计算过，和当前权重进行比较，取较小者
                        if curr_weight < cal.weight {
                            let w = Weight {
                                weight: curr_weight,
                                edge: e,
                            };
                            prev.insert(e.endid(), w.clone());
                            // 因为使用较小值修改了曾计算的值，需要将该节点
                            // 重新推入队列，导致其衍生的所有后续节点的重新
                            // 计算，这使得优先队列将存放更多的元素。
                            candidates.push(w);
                        }
                    } else {
                        // 未计算
                        let w = Weight {
                            weight: curr_weight,
                            edge: e,
                        };
                        prev.insert(e.endid(), w.clone());
                        if e.endid() == toid {
                            // 更新到达权重
                            reached = curr_weight;
                        } else {
                            // 推入堆中待计算
                            candidates.push(w);
                        }
                    }
                }
            }
        }
        let mut plan = vec![];
        let mut currid = toid;
        while currid != fromid {
            let w = &prev[&currid];
            currid = w.edge.startid();
            plan.push(w.edge);
        }
        plan
    }

    // 使用dfs生成遍历计划
    pub fn traverse(&self, centerid: u32, depth: u32) -> Vec<&E> {
        if !self.nodes.contains(centerid) || depth < 1 {
            return vec![];
        }
        let mut currid = centerid;
        let mut reached = HashSet::<u32>::new();
        reached.insert(currid);
        let mut candidates = Vec::<Depth<E>>::new();
        for exit in self.edges.exits(currid) {
            candidates.push(Depth {
                depth: 1,
                edge: exit,
            });
        }
        let mut plan = Vec::new();
        while let Some(d) = candidates.pop() {
            if reached.contains(&d.edge.endid()) {
                // 目标节点已路过，无需再走
                continue;
            }
            debug_assert!(d.depth <= depth);
            if currid == d.edge.startid() {
                // 当前房间就是来源房间，可直接添加到行走计划中
                plan.push(d.edge);
            } else {
                // 生成walk计划，并加入
                let mut walkplan = self.walk(currid, d.edge.endid());
                if walkplan.is_empty() {
                    log::warn!(
                        "failed to generate traverse plan because {} and {} are not connected",
                        currid,
                        d.edge.endid()
                    );
                    return vec![];
                }
                while let Some(step) = walkplan.pop() {
                    plan.push(step);
                }
            }
            // 将当前节点加入已访问列表
            reached.insert(currid);
            // 设置当前节点设置为目标节点
            currid = d.edge.endid();
            if d.depth < depth {
                // 在小于深度时将临近节点加入候选列表
                for exit in self.edges.exits(currid) {
                    candidates.push(Depth {
                        depth: d.depth + 1,
                        edge: exit,
                    });
                }
            }
        }
        plan.reverse();
        plan
    }
}

#[derive(Debug, Clone)]
struct Depth<'a, E> {
    depth: u32,
    edge: &'a E,
}

#[derive(Debug, Clone)]
struct Weight<'a, E> {
    weight: u32,
    edge: &'a E,
}

impl<'a, E> Ord for Weight<'a, E> {
    fn cmp(&self, other: &Self) -> Ordering {
        // 逆序
        other.weight.cmp(&self.weight)
    }
}

impl<'a, E> PartialOrd for Weight<'a, E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, E> PartialEq for Weight<'a, E> {
    fn eq(&self, other: &Self) -> bool {
        self.weight.eq(&other.weight)
    }
}

impl<'a, E> Eq for Weight<'a, E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Node;

    #[test]
    fn test_ref_clone() {
        #[derive(Clone)]
        struct CloneObj {}
        impl Drop for CloneObj {
            fn drop(&mut self) {
                println!("dropped");
            }
        }
        {
            let obj = CloneObj {};
            let w = Weight {
                weight: 1,
                edge: &obj,
            };
            let _w2 = w.clone();
        }
    }

    #[test]
    fn test_planner_simple_walk() {
        let mut nodes = Nodes::<N>::new();
        nodes.put(N { id: 1 });
        nodes.put(N { id: 2 });
        let mut edges = Edges::<E>::new();
        edges.insert(E {
            startid: 1,
            endid: 2,
            weight: 1,
        });
        let planner = Planner::new(&nodes, &edges);
        let rs = planner.walk(1, 2);
        println!("{:?}", rs);
        assert_eq!(
            vec![&E {
                startid: 1,
                endid: 2,
                weight: 1
            }],
            rs
        );
    }

    #[test]
    fn test_planner_complex_walk() {
        let mut nodes = Nodes::new();
        nodes.put(N { id: 1 });
        nodes.put(N { id: 2 });
        nodes.put(N { id: 3 });
        nodes.put(N { id: 4 });
        nodes.put(N { id: 5 });
        let mut edges = Edges::new();
        edges.insert(E {
            startid: 1,
            endid: 2,
            weight: 1,
        });
        edges.insert(E {
            startid: 2,
            endid: 3,
            weight: 1,
        });
        edges.insert(E {
            startid: 1,
            endid: 3,
            weight: 10,
        });
        edges.insert(E {
            startid: 3,
            endid: 4,
            weight: 1,
        });
        edges.insert(E {
            startid: 4,
            endid: 5,
            weight: 1,
        });
        edges.insert(E {
            startid: 3,
            endid: 5,
            weight: 10,
        });
        let planner = Planner::new(&nodes, &edges);
        let rs = planner.walk(1, 5);
        println!("{:?}", rs);
        assert_eq!(
            vec![
                &E {
                    startid: 4,
                    endid: 5,
                    weight: 1
                },
                &E {
                    startid: 3,
                    endid: 4,
                    weight: 1
                },
                &E {
                    startid: 2,
                    endid: 3,
                    weight: 1
                },
                &E {
                    startid: 1,
                    endid: 2,
                    weight: 1
                },
            ],
            rs
        );
    }

    #[test]
    fn test_planner_simple_traverse() {
        let mut nodes = Nodes::<N>::new();
        nodes.put(N { id: 1 });
        nodes.put(N { id: 2 });
        nodes.put(N { id: 3 });
        let mut edges = Edges::<E>::new();
        edges.insert(E {
            startid: 1,
            endid: 2,
            weight: 1,
        });
        edges.insert(E {
            startid: 2,
            endid: 3,
            weight: 1,
        });
        edges.insert(E {
            startid: 3,
            endid: 1,
            weight: 1,
        });
        let planner = Planner::new(&nodes, &edges);
        let mut rs = planner.traverse(1, 2);
        rs.reverse();
        println!("{:?}", rs);
        assert_eq!(
            vec![
                &E {
                    startid: 1,
                    endid: 2,
                    weight: 1
                },
                &E {
                    startid: 2,
                    endid: 3,
                    weight: 1
                },
            ],
            rs
        );
    }

    #[test]
    fn test_planner_complex_traverse() {
        let mut nodes = Nodes::<N>::new();
        nodes.put(N { id: 1 });
        nodes.put(N { id: 2 });
        nodes.put(N { id: 3 });
        nodes.put(N { id: 4 });
        nodes.put(N { id: 5 });
        nodes.put(N { id: 6 });
        let mut edges = Edges::<E>::new();
        // 1 -- 2 -- 3
        // |
        // 4 -- 5
        // |
        // 6
        edges.insert(E {
            startid: 1,
            endid: 2,
            weight: 1,
        });
        edges.insert(E {
            startid: 2,
            endid: 1,
            weight: 1,
        });
        edges.insert(E {
            startid: 2,
            endid: 3,
            weight: 1,
        });
        edges.insert(E {
            startid: 3,
            endid: 2,
            weight: 1,
        });
        edges.insert(E {
            startid: 1,
            endid: 4,
            weight: 1,
        });
        edges.insert(E {
            startid: 4,
            endid: 1,
            weight: 1,
        });
        edges.insert(E {
            startid: 4,
            endid: 5,
            weight: 1,
        });
        edges.insert(E {
            startid: 5,
            endid: 4,
            weight: 1,
        });
        edges.insert(E {
            startid: 4,
            endid: 6,
            weight: 1,
        });
        edges.insert(E {
            startid: 6,
            endid: 4,
            weight: 1,
        });
        let planner = Planner::new(&nodes, &edges);
        let mut rs = planner.traverse(1, 2);
        rs.reverse();
        println!("{:#?}", rs);
        for (prev, next) in rs.iter().zip(rs.iter().skip(1)) {
            assert_eq!(prev.endid(), next.startid());
        }
        let mut rs = planner.traverse(1, 1);
        rs.reverse();
        println!("{:#?}", rs);
        assert_eq!(3, rs.len());
        for (prev, next) in rs.iter().zip(rs.iter().skip(1)) {
            assert_eq!(prev.endid(), next.startid());
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct E {
        startid: u32,
        endid: u32,
        weight: u32,
    }

    impl Edge for E {
        fn pseudo(id: u32) -> Self {
            Self {
                startid: id,
                endid: id,
                weight: 0,
            }
        }
        fn startid(&self) -> u32 {
            self.startid
        }
        fn endid(&self) -> u32 {
            self.endid
        }
        fn weight(&self) -> u32 {
            self.weight
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct N {
        id: u32,
    }

    impl Node for N {
        fn id(&self) -> u32 {
            self.id
        }
    }
}
