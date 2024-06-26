use crate::geometry::{bounding_sphere, circumsphere, in_sphere};
use crate::scheduler::{find_placement, make_queue};
use dashmap::DashMap;
use kiddo::{KdTree, SquaredEuclidean};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelExtend,
    ParallelIterator,
};

#[derive(Debug, Clone)]
pub struct Simplex<const N: usize, const M: usize> {
    pub vertices: [usize; M],
    pub center: [f64; N],
    pub radius: f64,
    pub neighbors: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct Vertex<const N: usize> {
    pub coordinates: [f64; N],
    pub simplex: Vec<usize>,
}

pub struct DelaunayTree<const N: usize, const M: usize> {
    // Make sure M = N + 1
    pub kdtree: KdTree<f64, N>,
    pub vertices: DashMap<usize, Vertex<N>>,
    pub simplices: DashMap<usize, Simplex<N, M>>,
    pub max_simplex_id: usize,
}

impl<const N: usize, const M: usize> DelaunayTree<N, M> {
    pub fn locate(&self, vertex: [f64; N]) -> Vec<usize> {
        let mut output: Vec<usize> = vec![];
        let simplex_id = &self
            .vertices
            .get(&(self.kdtree.nearest_one::<SquaredEuclidean>(&vertex).item as usize))
            .unwrap()
            .simplex;
        for id in simplex_id {
            let _simplex = self.simplices.get(&id).unwrap();
            if in_sphere(vertex, _simplex.center, _simplex.radius) {
                output.push(*id);
                output = self.find_all_neighbors(&mut output, *id, vertex);
            }
        }
        if output.len() == 0 {
            println!("Simplex_id {:?}", simplex_id);
            println!(
                "{:?}",
                self.kdtree.nearest_one::<SquaredEuclidean>(&vertex).item as usize
            );
            panic!("No simplex found for vertex {:?}", vertex);
        }
        output.sort();
        output.dedup();
        output
    }

    fn find_all_neighbors(
        &self,
        output: &mut Vec<usize>,
        node_id: usize,
        vertex: [f64; N],
    ) -> Vec<usize> {
        let neighbors = &self.simplices.get(&node_id).unwrap().neighbors;
        for neighbor in neighbors {
            let _simplex = self.simplices.get(&neighbor).unwrap();
            if !output.contains(&neighbor) && in_sphere(vertex, _simplex.center, _simplex.radius) {
                output.push(*neighbor);
                self.find_all_neighbors(output, *neighbor, vertex);
            }
        }
        output.to_vec()
    }

    pub fn get_new_simplices(
        &self,
        killed_site_id: usize,
        vertex: [f64; N],
        vertex_id: usize,
    ) -> (
        Vec<[usize; M]>,
        Vec<[f64; N]>,
        Vec<f64>,
        Vec<(usize, usize)>,
    ) {
        let mut simplices: Vec<[usize; M]> = vec![];
        let mut simplices_id: Vec<usize> = vec![];
        let mut centers: Vec<[f64; N]> = vec![];
        let mut radii: Vec<f64> = vec![];
        let mut neighbors: Vec<(usize, usize)> = vec![];

        let _killed_simplex = &self.simplices.get(&killed_site_id).unwrap();

        let killed_site: [usize; M] = _killed_simplex.vertices;
        for neighbor_id in _killed_simplex.neighbors.iter() {
            let neighbor_simplex = &self.simplices.get(neighbor_id).unwrap();
            if !in_sphere(vertex, neighbor_simplex.center, neighbor_simplex.radius) {
                let mut new_simplex = [0; M];
                new_simplex[0] = vertex_id;
                let mut count = 1;
                for i in 0..M {
                    if killed_site.contains(&neighbor_simplex.vertices[i]) {
                        new_simplex[count] = neighbor_simplex.vertices[i];
                        count += 1;
                    }
                }
                let mut new_simplex_vertex: [[f64; N]; M] = [[0.0; N]; M];
                new_simplex_vertex[0] = vertex.clone();
                for i in 1..M {
                    new_simplex_vertex[i] = self.vertices.get(&new_simplex[i]).unwrap().coordinates;
                }
                let (center, radius) = circumsphere(new_simplex_vertex);
                simplices.push(new_simplex);
                simplices_id.push(self.max_simplex_id + simplices.len());
                centers.push(center);
                radii.push(radius);
                neighbors.push((*neighbor_id, killed_site_id));
            }
        }
        (simplices, centers, radii, neighbors)
    }

    pub fn insert_point(&mut self, update: &TreeUpdate<N, M>) {
        // Insert one point in the tree
        // This does not parallelize the insert so we don't have to pay for overhead.
        // Works well for small number of points
        let killed_sites = &update.killed_sites;
        self.kdtree.add(&update.vertex, self.vertices.len() as u64);

        // Update simplices
        self.simplices
            .extend(update.simplices.iter().enumerate().map(|(i, simplex)| {
                let current_id = self.max_simplex_id + update.simplices_id[i];
                let _simplex = Simplex {
                    vertices: *simplex,
                    center: update.centers[i],
                    radius: update.radii[i],
                    neighbors: vec![update.neighbors[i].0],
                };
                (current_id, _simplex)
            }));

        // update neighbor relations

        update
            .neighbors
            .iter()
            .enumerate()
            .for_each(|(i, (neighbor_id, killed_id))| {
                let mut neighbor = self.simplices.get_mut(neighbor_id).unwrap();
                for j in 0..neighbor.neighbors.len() {
                    if neighbor.neighbors[j] == *killed_id {
                        neighbor.neighbors[j] = self.max_simplex_id + update.simplices_id[i];
                    }
                }
            });

        update
            .new_neighbors
            .iter()
            .for_each(|(new_neighbor_id1, new_neighbor_id2)| {
                self.simplices
                    .get_mut(&(self.max_simplex_id + *new_neighbor_id1))
                    .unwrap()
                    .neighbors
                    .push(self.max_simplex_id + *new_neighbor_id2);
            });

        // Update vertices_simplex

        self.vertices.insert(
            self.vertices.len(),
            Vertex {
                coordinates: update.vertex,
                simplex: vec![],
            },
        );

        update
            .simplices
            .iter()
            .enumerate()
            .for_each(|(i, simplex)| {
                for j in 0..M {
                    self.vertices
                        .get_mut(&(*simplex)[j])
                        .unwrap()
                        .simplex
                        .push(self.max_simplex_id + update.simplices_id[i]);
                }
            });

        killed_sites.iter().for_each(|killed_sites_id| {
            for i in 0..M {
                self.vertices
                    .get_mut(&self.simplices.get(killed_sites_id).unwrap().vertices[i])
                    .unwrap()
                    .simplex
                    .retain(|&x| x != *killed_sites_id);
            }
        });

        // Remove killed sites
        killed_sites.iter().for_each(|killed_sites_id| {
            self.simplices.remove(killed_sites_id);
        });

        self.max_simplex_id += update.simplices.len();
    }

    pub fn insert_points_parallel(&mut self, updates: &Vec<TreeUpdate<N, M>>) {
        let mut simplices_length: Vec<usize> = vec![];
        simplices_length.par_extend(
            updates
                .par_iter()
                .map(|update| update.simplices.len())
                .collect::<Vec<usize>>(),
        );
        simplices_length.iter_mut().fold(0, |acc, x| {
            *x += acc;
            *x
        });
        simplices_length.insert(0, 0);
        let length = self.vertices.len();

        updates.iter().enumerate().for_each(|(i, update)| {
            self.kdtree
                .add(&update.vertex, (self.vertices.len() + i) as u64);
        });

        updates
            .par_iter()
            .enumerate()
            .for_each(|(update_index, update)| {
                let killed_sites = &update.killed_sites;

                // Update simplices
                update
                    .simplices
                    .par_iter()
                    .enumerate()
                    .for_each(|(i, simplex)| {
                        let current_id = self.max_simplex_id
                            + update.simplices_id[i]
                            + simplices_length[update_index];
                        let _simplex = Simplex {
                            vertices: *simplex,
                            center: update.centers[i],
                            radius: update.radii[i],
                            neighbors: vec![update.neighbors[i].0],
                        };
                        self.simplices.insert(current_id, _simplex);
                    });

                // update neighbor relations

                update
                    .neighbors
                    .iter()
                    .enumerate()
                    .for_each(|(i, (neighbor_id, killed_id))| {
                        let mut neighbor = self.simplices.get_mut(neighbor_id).unwrap();
                        for j in 0..neighbor.neighbors.len() {
                            if neighbor.neighbors[j] == *killed_id {
                                neighbor.neighbors[j] = self.max_simplex_id
                                    + update.simplices_id[i]
                                    + simplices_length[update_index];
                            }
                        }
                    });

                update
                    .new_neighbors
                    .iter()
                    .for_each(|(new_neighbor_id1, new_neighbor_id2)| {
                        self.simplices
                            .get_mut(
                                &(self.max_simplex_id
                                    + *new_neighbor_id1
                                    + simplices_length[update_index]),
                            )
                            .unwrap()
                            .neighbors
                            .push(
                                self.max_simplex_id
                                    + *new_neighbor_id2
                                    + simplices_length[update_index],
                            );
                    });

                // Update vertices_simplex

                self.vertices.insert(
                    length + update_index,
                    Vertex {
                        coordinates: update.vertex,
                        simplex: vec![],
                    },
                );

                update
                    .simplices
                    .iter()
                    .enumerate()
                    .for_each(|(i, simplex)| {
                        for j in 0..M {
                            self.vertices
                                .get_mut(&(*simplex)[j])
                                .unwrap()
                                .simplex
                                .push(self.max_simplex_id + update.simplices_id[i] + simplices_length[update_index]);
                        }
                    });

                killed_sites.iter().for_each(|killed_sites_id| {
                    for i in 0..M {
                        self.vertices
                            .get_mut(&self.simplices.get(killed_sites_id).unwrap().vertices[i])
                            .unwrap()
                            .simplex
                            .retain(|&x| x != *killed_sites_id);
                    }
                });

                // Remove killed sites
                killed_sites.iter().for_each(|killed_sites_id| {
                    self.simplices.remove(killed_sites_id);
                });
            });

        self.max_simplex_id += simplices_length.last().unwrap();
    }

    pub fn add_points_to_tree(&mut self, vertices: Vec<[f64; N]>) {
        println!("Making queue and finding placement");
        let start = std::time::Instant::now();
        let queue = make_queue(vertices, self);
        let placement = find_placement(&queue);
        let mut batches = vec![];
        batches.par_extend(
            (1..placement.iter().max().unwrap() + 1)
                .into_par_iter()
                .map(|i| {
                    queue
                        .iter()
                        .enumerate()
                        .filter(|(id, _)| placement[*id] == i)
                        .collect::<Vec<(usize, &(usize, [f64; N], Vec<usize>))>>()
                }),
        );
        println!("Queue and placement finished in {:?}", start.elapsed());
        #[cfg(debug_assertions)]
        {
            let time = std::time::Instant::now();
            for batch in batches {
                let n_points = self.vertices.len();
                println!("Valid batch {:?}", batch.len());
                let updates = batch
                    .par_iter()
                    .enumerate()
                    // .with_min_len(8)
                    .map(|(id, vertex)| TreeUpdate::new(n_points + id, vertex.1 .1, self))
                    .collect::<Vec<TreeUpdate<N, M>>>();
                self.insert_points_parallel(&updates);
            }
            println!("Insertion finished in {:?}", time.elapsed());
        }
        #[cfg(not(debug_assertions))]
        {
            for batch in batches {
                let n_points = self.vertices.len();
                let updates = batch
                    .par_iter()
                    .enumerate()
                    // .with_min_len(16)
                    .map(|(id, vertex)| TreeUpdate::new(n_points + id, vertex.1 .1, self))
                    .collect::<Vec<TreeUpdate<N, M>>>();
                // for update in updates {
                //     self.insert_point(&update);
                // }
                self.insert_points_parallel(&updates);
            }
        }
    }
}

impl DelaunayTree<3, 4> {
    pub fn new(vertices: Vec<[f64; 3]>) -> Self {
        // Turn vertices into nalgebra points
        let (center, mut radius) = bounding_sphere(vertices);
        radius *= 10.0;

        let first_vertex = [center[0], center[1], center[2] + radius];
        let second_vertex = [center[0] + radius, center[1], center[2] - radius];
        let third_vertex = [
            center[0] + radius * (2. * std::f64::consts::PI / 3.).cos(),
            center[1] + radius * (2. * std::f64::consts::PI / 3.).sin(),
            center[2] - radius,
        ];
        let fourth_vertex = [
            center[0] + radius * (4. * std::f64::consts::PI / 3.).cos(),
            center[1] + radius * (4. * std::f64::consts::PI / 3.).sin(),
            center[2] - radius,
        ];
        let ghost_vertex = [
            [0. + center[0], 0. + center[1], radius + center[2]],
            [0. + center[0], 0. + center[1], radius + center[2]],
            [0. + center[0], 0. + center[1], radius + center[2]],
            [center[0] + radius, center[1], center[2] - radius],
        ];

        let mut kdtree = KdTree::new();

        kdtree.add(&first_vertex, 0);
        kdtree.add(&second_vertex, 1);
        kdtree.add(&third_vertex, 2);
        kdtree.add(&fourth_vertex, 3);
        for (i, vertex) in ghost_vertex.iter().enumerate() {
            kdtree.add(vertex, (i + 4) as u64);
        }

        let vertex = DashMap::new();

        let vertices = vec![
            first_vertex,
            second_vertex,
            third_vertex,
            fourth_vertex,
            ghost_vertex[0],
            ghost_vertex[1],
            ghost_vertex[2],
            ghost_vertex[3],
        ];
        let vertices_simplex = [
            vec![0, 1, 2, 3],
            vec![0, 1, 3, 4],
            vec![0, 1, 2, 4],
            vec![0, 2, 3, 4],
            vec![1],
            vec![2],
            vec![3],
            vec![4],
        ]
        .to_vec();
        for i in 0..8 {
            vertex.insert(
                i,
                Vertex {
                    coordinates: vertices[i],
                    simplex: vertices_simplex[i].clone(),
                },
            );
        }

        let simplices = DashMap::new();
        simplices.insert(
            0,
            Simplex {
                vertices: [0, 1, 2, 3],
                center,
                radius,
                neighbors: vec![1, 2, 3, 4],
            },
        );
        simplices.insert(
            1,
            Simplex {
                vertices: [4, 0, 1, 2],
                center: [0., 0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        simplices.insert(
            2,
            Simplex {
                vertices: [5, 0, 2, 3],
                center: [0., 0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        simplices.insert(
            3,
            Simplex {
                vertices: [6, 0, 3, 1],
                center: [0., 0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        simplices.insert(
            4,
            Simplex {
                vertices: [7, 1, 2, 3],
                center: [0., 0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        let delaunay_tree = DelaunayTree {
            kdtree,
            vertices: vertex,
            simplices,
            max_simplex_id: 4,
        };
        delaunay_tree
    }

    pub fn check_delaunay(&self) -> bool {
        let mut result = true;
        for simplex in self.simplices.iter() {
            for vertex in self.vertices.iter() {
                let local_simplex = self.simplices.get(simplex.key()).unwrap();
                if in_sphere(
                    vertex.coordinates,
                    local_simplex.center,
                    local_simplex.radius,
                ) && !local_simplex.vertices.contains(&vertex.key())
                    && local_simplex.vertices.iter().all(|&x| x > 7)
                // TODO fix this
                {
                    result = false;
                    println!(
                        "Vertex {:?} is in sphere of simplex {:?}",
                        vertex.key(),
                        simplex.key()
                    );
                    println!("Vertices coordinates {:?}", vertex.coordinates);
                    for i in 0..4 {
                        println!("Simplex vertices {:?}", local_simplex.vertices[i]);
                    }
                    println!("Center of simplex {:?}", local_simplex.center);
                    println!("Radius of simplex {:?}", local_simplex.radius);
                }
            }
        }
        result
    }
}

impl DelaunayTree<2, 3> {
    pub fn new(vertices: Vec<[f64; 2]>) -> Self {
        // Turn vertices into nalgebra points
        let (center, mut radius) = bounding_sphere(vertices);
        radius *= 10.0;

        let first_vertex = [center[0] + radius, center[1]];
        let second_vertex = [
            center[0] + radius * (2. * std::f64::consts::PI / 3.).cos(),
            center[1] + radius * (2. * std::f64::consts::PI / 3.).sin(),
        ];
        let third_vertex = [
            center[0] + radius * (4. * std::f64::consts::PI / 3.).cos(),
            center[1] + radius * (4. * std::f64::consts::PI / 3.).sin(),
        ];
        let vertices = vec![
            first_vertex,
            second_vertex,
            third_vertex,
            first_vertex.clone(),
            second_vertex.clone(),
            third_vertex.clone(),
        ];

        println!("{:?}", vertices);
        let mut kdtree = KdTree::new();

        for i in 0..6 {
            kdtree.add(&vertices[i], i as u64);
        }

        let vertices_simplex = [
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![0, 2, 3],
            vec![1],
            vec![2],
            vec![3],
        ]
        .to_vec();

        let vertex = DashMap::new();
        for i in 0..6 {
            vertex.insert(
                i,
                Vertex {
                    coordinates: vertices[i],
                    simplex: vertices_simplex[i].clone(),
                },
            );
        }

        let simplices = DashMap::new();
        simplices.insert(
            0,
            Simplex {
                vertices: [0, 1, 2],
                center,
                radius,
                neighbors: vec![1, 2, 3],
            },
        );
        simplices.insert(
            1,
            Simplex {
                vertices: [3, 0, 1],
                center: [0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        simplices.insert(
            2,
            Simplex {
                vertices: [4, 0, 2],
                center: [0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        simplices.insert(
            3,
            Simplex {
                vertices: [5, 1, 2],
                center: [0., 0.],
                radius: 0.,
                neighbors: vec![0],
            },
        );
        let delaunay_tree = DelaunayTree::<2, 3> {
            kdtree,
            vertices: vertex,
            simplices,
            max_simplex_id: 3,
        };
        delaunay_tree
    }

    pub fn check_delaunay(&self) -> bool {
        let mut result = true;
        for simplex in self.simplices.iter() {
            for vertex in self.vertices.iter() {
                let local_simplex = self.simplices.get(simplex.key()).unwrap();
                if in_sphere(
                    vertex.coordinates,
                    local_simplex.center,
                    local_simplex.radius,
                ) && !local_simplex.vertices.contains(&vertex.key())
                    && local_simplex.vertices.iter().all(|&x| x > 5)
                // TODO fix this
                {
                    result = false;
                    println!(
                        "Vertex {:?} is in sphere of simplex {:?}",
                        vertex.key(),
                        simplex.key()
                    );
                    println!("Vertices coordinates {:?}", vertex.coordinates);
                    for i in 0..4 {
                        println!("Simplex vertices {:?}", local_simplex.vertices[i]);
                    }
                    println!("Center of simplex {:?}", local_simplex.center);
                    println!("Radius of simplex {:?}", local_simplex.radius);
                }
            }
        }
        result
    }
}

fn pair_simplices<const N: usize, const M: usize>(
    simplices: &Vec<[usize; M]>,
    simplices_id: &Vec<usize>,
) -> Vec<(usize, usize)> {
    let mut new_neighbors: Vec<(usize, usize)> = vec![];
    let n_simplices = simplices.len();
    for i in 0..n_simplices {
        for j in (i + 1)..n_simplices {
            let mut count = 0;
            for k in 0..M {
                if simplices[i].contains(&simplices[j][k]) {
                    count += 1;
                }
            }
            if count == N {
                new_neighbors.push((simplices_id[i], simplices_id[j]));
                new_neighbors.push((simplices_id[j], simplices_id[i]));
            }
        }
    }
    new_neighbors
}

#[derive(Debug, Clone)]
pub struct TreeUpdate<const N: usize, const M: usize> {
    vertex: [f64; N],
    killed_sites: Vec<usize>,
    simplices: Vec<[usize; M]>,
    simplices_id: Vec<usize>,
    centers: Vec<[f64; N]>,
    radii: Vec<f64>,
    neighbors: Vec<(usize, usize)>,
    new_neighbors: Vec<(usize, usize)>,
}

impl<const N: usize, const M: usize> TreeUpdate<N, M> {
    pub fn new(id: usize, vertex: [f64; N], tree: &DelaunayTree<N, M>) -> Self {
        let killed_sites = tree.locate(vertex);
        let mut simplices: Vec<[usize; M]> = vec![];
        let mut centers: Vec<[f64; N]> = vec![];
        let mut radii: Vec<f64> = vec![];
        let mut neighbors: Vec<(usize, usize)> = vec![];

        for i in 0..killed_sites.len() {
            let (simplices_, centers_, radii_, neighbors_) =
                tree.get_new_simplices(killed_sites[i], vertex, id);
            simplices.extend(simplices_);
            centers.extend(centers_);
            radii.extend(radii_);
            neighbors.extend(neighbors_);
        }

        let simplices_id = (1..simplices.len() + 1).collect::<Vec<usize>>();
        let new_neighbors: Vec<(usize, usize)> = pair_simplices::<N, M>(&simplices, &simplices_id);

        TreeUpdate {
            vertex,
            killed_sites,
            simplices,
            simplices_id,
            centers,
            radii,
            neighbors,
            new_neighbors,
        }
    }
}
