using Revise
using BoundingSphere
using LinearAlgebra
using Plots
plotlyjs()
using Random
include("Primitives.jl")

function plot_simplex_2d(simplex::DelaunayTreeNode, vertices::Dict{Int, Vertex})
    x, y = [], []
    for vertex_id in [1,2,3,1]
        push!(x, vertices[simplex.vertices[vertex_id]].position[1])
        push!(y, vertices[simplex.vertices[vertex_id]].position[2])
    end
    return x, y
end

function plot_simplex_3d(simplex::DelaunayTreeNode, vertices::Dict{Int, Vertex})
    x, y, z = [], [], []
    for vertex_id in [1,2,3,1,4,3,2,4]
        push!(x, vertices[simplex.vertices[vertex_id]].position[1])
        push!(y, vertices[simplex.vertices[vertex_id]].position[2])
        push!(z, vertices[simplex.vertices[vertex_id]].position[3])
    end
    return x, y, z
end

function circumcircle(node_id::Int, tree::DelaunayTree)
    vertices = map(x->tree.vertices[x].position, tree.simplices[node_id].vertices)
    x1, y1 = vertices[1]
    x2, y2 = vertices[2]
    x3, y3 = vertices[3]

    # Midpoints of AB and BC
    D = ((x1 + x2) / 2, (y1 + y2) / 2)
    E = ((x2 + x3) / 2, (y2 + y3) / 2)

    # Slopes of AB and BC
    mAB = (y2 - y1) / (x2 - x1)
    mBC = (y3 - y2) / (x3 - x2)

    # Slopes of perpendicular bisectors
    mD = -1 / mAB
    mE = -1 / mBC

    # Calculating the circumcenter (X, Y)
    X = (mD * D[1] - mE * E[1] + E[2] - D[2]) / (mD - mE)
    Y = mD * (X - D[1]) + D[2]

    # Radius of the circumcircle
    R = sqrt((X - x1)^2 + (Y - y1)^2)

    return ((X, Y), R)
end

function circumsphere(node_id::Int, tree::DelaunayTree)
    vertices = map(x->tree.vertices[x].position, tree.simplices[node_id].vertices)
    # Convert vertices into Julia tuples if they're not already
    x1, y1, z1 = vertices[1]
    x2, y2, z2 = vertices[2]
    x3, y3, z3 = vertices[3]
    x4, y4, z4 = vertices[4]

    # Matrix of quadratic form
    M = [x2-x1 y2-y1 z2-z1 ((x2^2 - x1^2) + (y2^2 - y1^2) + (z2^2 - z1^2)) / 2;
         x3-x1 y3-y1 z3-z1 ((x3^2 - x1^2) + (y3^2 - y1^2) + (z3^2 - z1^2)) / 2;
         x4-x1 y4-y1 z4-z1 ((x4^2 - x1^2) + (y4^2 - y1^2) + (z4^2 - z1^2)) / 2]

    # Solve the linear system
    center = M \ [1; 1; 1] # Solves M * [X; Y; Z] = [1; 1; 1]

    # Radius is the distance from the center to any vertex, here A is chosen
    R = sqrt((center[1] - x1)^2 + (center[2] - y1)^2 + (center[3] - z1)^2)

    return (center..., R) # Return the center coordinates and the radius
end


function initialize_tree_3d(points::Vector{Vertex})::DelaunayTree
    positions = map(x -> x.position, points)
    center, radius = boundingsphere(positions)
    radius = radius*3
    first_vertex = Vertex(-8, center + [0, 0, radius])
    second_vertex = Vertex(-7, center + [radius * cos(0), radius * sin(0), -radius ])
    third_vertex = Vertex(-6, center + [radius * cos(2 * pi / 3), radius * sin(2 * pi / 3), -radius])
    fourth_vertex = Vertex(-5, center + [radius * cos(4 * pi / 3), radius * sin(4 * pi / 3), -radius])
    ghost_vertex = [Vertex(-4, center + [0, 0, radius]), Vertex(-3, center + [0, 0, radius]), Vertex(-2, center + [0, 0, radius]), Vertex(-1, center + [radius  * cos(0), radius * sin(0), -radius])]
    verticies = [first_vertex, second_vertex, third_vertex, fourth_vertex, ghost_vertex...]
    verticies = Dict(map(x -> x.id => x, verticies))
    node = DelaunayTreeNode(1, false, [-8, -7, -6, -5])
    unbounded_node = Vector{DelaunayTreeNode}()
    push!(unbounded_node, DelaunayTreeNode(2, false, [-4, -8, -7, -6]))
    push!(unbounded_node, DelaunayTreeNode(3, false, [-3, -8, -6, -5]))
    push!(unbounded_node, DelaunayTreeNode(4, false, [-2, -8, -5, -7]))
    push!(unbounded_node, DelaunayTreeNode(5, false, [-1, -7, -6, -5]))
    nodes = Dict(1 => node, 2 => unbounded_node[1], 3 => unbounded_node[2], 4 => unbounded_node[3], 5 => unbounded_node[4])
    children_relation = Dict(1 => [], 2 => [], 3 => [], 4 => [], 5 => [])
    step_children_relation = Dict(1 => Dict{Vector{Int},Vector{Int}}(), 2 => Dict{Vector{Int},Vector{Int}}(), 3 => Dict{Vector{Int},Vector{Int}}(), 4 => Dict{Vector{Int},Vector{Int}}(), 5 => Dict{Vector{Int},Vector{Int}}())
    neighbors_relation = Dict(1 => [2, 3, 4, 5], 2 => [1], 3 => [1], 4 => [1], 5 => [1])
    return DelaunayTree(verticies, nodes, children_relation, step_children_relation, neighbors_relation)
end

function initialize_tree_2d(points::Vector{Vertex})::DelaunayTree
    positions = map(x -> x.position, points)
    center, radius = boundingsphere(positions)
    radius = radius*3
    first_vertex = Vertex(-6, center + [radius * cos(0* pi / 3), radius * sin(0 * pi / 3)])
    second_vertex = Vertex(-5, center + [radius * cos(2 * pi / 3), radius * sin(2 * pi / 3)])
    third_vertex = Vertex(-4, center + [radius * cos(4 * pi / 3), radius * sin(4 * pi / 3)])
    ghost_vertex = [Vertex(-3, center + [radius * cos(0 * pi / 3), radius * sin(0 * pi / 3)]), Vertex(-2, center + [radius * cos(2 * pi / 3), radius * sin(2 * pi / 3)]), Vertex(-1, center + [radius * cos(4 * pi / 3), radius * sin(4 * pi / 3)])]
    verticies = [first_vertex, second_vertex, third_vertex, ghost_vertex...]
    verticies = Dict(map(x -> x.id => x, verticies))
    node = DelaunayTreeNode(1, false, [-6, -5, -4])
    unbounded_node = Vector{DelaunayTreeNode}()
    push!(unbounded_node, DelaunayTreeNode(2, false, [-3, -6, -5]))
    push!(unbounded_node, DelaunayTreeNode(3, false, [-1, -6, -4]))
    push!(unbounded_node, DelaunayTreeNode(4, false, [-2, -5, -4]))
    nodes = Dict(1 => node, 2 => unbounded_node[1], 3 => unbounded_node[2], 4 => unbounded_node[3])
    children_relation = Dict(1 => [], 2 => [], 3 => [], 4 => [])
    step_children_relation = Dict(1 => Dict{Vector{Int},Vector{Int}}(), 2 => Dict{Vector{Int},Vector{Int}}(), 3 => Dict{Vector{Int},Vector{Int}}(), 4 => Dict{Vector{Int},Vector{Int}}())
    neighbors_relation = Dict(1 => [2, 3, 4], 2 => [1], 3 => [1], 4 => [1])
    return DelaunayTree(verticies, nodes, children_relation, step_children_relation, neighbors_relation)
end

function in_sphere(node_id::Int, point::Vertex, tree::DelaunayTree; n_dims::Int=3)::Bool
    # position = reduce(hcat, map(x -> x.position, map(x->tree.vertices[x], tree.simplices[node_id].vertices))) .- point.position
    # position = vcat(position, mapslices(norm, position, dims=1))
    # if n_dims == 3
    #     return det(position) < 1e-15
    # elseif n_dims == 2
    #     vertices = map(x->x.position, map(x->tree.vertices[x], tree.simplices[node_id].vertices))
    #     sign_area = sign(vertices[1][1]*(vertices[2][2]-vertices[3][2]) + vertices[2][1]*(vertices[3][2]-vertices[1][2]) + vertices[3][1]*(vertices[1][2]-vertices[2][2]))
    #     if sign_area > 0
    #         return det(position) > 1e-15
    #     else
    #         return det(position) < -1e-15
    #     end
    # end
    if n_dims==3
        center, radius = circumsphere(node_id, tree)
        return norm(point.position .- center) < radius
    elseif n_dims==2
        center, radius = circumcircle(node_id, tree)
        return norm(point.position .- center) < radius
    end
end

function locate(visited_ids::Vector{Int}, output::Vector{Int}, vertex::Vertex, current_node_id::Int, tree::DelaunayTree; n_dims::Int = 3)::Vector{Int}
    if current_node_id ∉ visited_ids && in_sphere(current_node_id, vertex, tree, n_dims=n_dims)
        push!(visited_ids, current_node_id)
        push!(output, current_node_id)
        childrens = tree.children_relation[current_node_id]
        for child_id in childrens
            locate(visited_ids, output, vertex, child_id, tree, n_dims=n_dims)
        end
        step_childrens = collect(values(tree.step_children_relation[current_node_id]))
        if length(step_childrens) > 0
            step_childrens = vcat(step_childrens...)
        end
        for step_children_id in step_childrens
            locate(visited_ids, output, vertex, step_children_id, tree, n_dims=n_dims)
        end
        return output
    else
        return output
    end
end

function common_facet(simplex1::DelaunayTreeNode, simplex2::DelaunayTreeNode; n_dims::Int = 3)::Vector{Int}
    common = intersect(simplex1.vertices, simplex2.vertices)
    if length(common) == n_dims
        return common
    else
        return []
    end
end

function insert_point(tree::DelaunayTree, point::Vertex; n_dims::Int=3)
    killed_nodes = locate(Vector{Int}(), Vector{Int}(), point, 1, tree, n_dims=n_dims)
    println("killed_nodes: ", killed_nodes)
    new_node_id = Vector{Int}()
    for node_id in killed_nodes
        if !tree.simplices[node_id].dead
            tree.simplices[node_id].dead = true
            for neighbor_id in tree.neighbors_relation[node_id]
                println(in_sphere(neighbor_id, point, tree, n_dims=n_dims))
                if !in_sphere(neighbor_id, point, tree, n_dims=n_dims)
                    print("neighbor_id: ", neighbor_id)
                    facet = common_facet(tree.simplices[node_id], tree.simplices[neighbor_id], n_dims=n_dims)
                    println("facet: ", facet)
                    if length(facet) == n_dims
                        # Creating new node
                        new_id = length(tree.simplices) + 1
                        println("new_id: ", new_id)
                        println("facet: ", facet)
                        new_node = DelaunayTreeNode(new_id, false, [point.id, facet...])
                        tree.simplices[new_id] = new_node
                        push!(new_node_id, new_node.id)
                        tree.children_relation[new_node.id] = Vector{Int}()
                        tree.step_children_relation[new_node.id] = Dict{Vector{Int}, Vector{Int}}()
                        tree.neighbors_relation[new_node.id] = [neighbor_id]

                        # Updating parent relationship
                        push!(tree.children_relation[node_id],new_node.id)
                        if haskey(tree.step_children_relation[neighbor_id], facet)
                            push!(tree.step_children_relation[neighbor_id][facet], new_node.id)
                        else
                            tree.step_children_relation[neighbor_id][facet] = [new_node.id]
                        end

                        # Updating neighbor relationship for the neighbor of the killed node
                        killed_node_id = findfirst(x->x==node_id, tree.neighbors_relation[neighbor_id])
                        if killed_node_id !== nothing
                            tree.neighbors_relation[neighbor_id][findfirst(x->x==node_id, tree.neighbors_relation[neighbor_id])] = new_node.id
                        end
                    end
                end
            end
        end
    end
    for i in 1:length(new_node_id)
        for j in i+1:length(new_node_id)
            new_id1 = new_node_id[i]
            new_id2 = new_node_id[j]
            facet = common_facet(tree.simplices[new_id1], tree.simplices[new_id2], n_dims=n_dims)
            if length(facet) == n_dims
                push!(tree.neighbors_relation[new_id1], new_id2)
                push!(tree.neighbors_relation[new_id2], new_id1)
            end
        end
    end
    tree.vertices[point.id] = point
end

Random.seed!(123)
n = 5
n_dims = 2
test_points = initialize_vertex(n, n_dims=n_dims)
tree = initialize_tree_2d(test_points)

for point in test_points
    insert_point(tree, point, n_dims=n_dims)
end

x,y = plot_simplex_2d(tree.simplices[1], tree.vertices)
plot(x, y, label="Points", size=(800, 800))
for i in 2:length(tree.simplices)
    if !tree.simplices[i].dead && all(tree.simplices[i].vertices.>0)
        x,y = plot_simplex_2d(tree.simplices[i], tree.vertices)
        plot!(x, y, label="Points", size=(800, 800))
    end
end

scatter!([x for x in map(x -> x.position[1], test_points)], [y for y in map(x -> x.position[2], test_points)], label="Points", color=["red","blue","green", "yellow", "black"])

function check_delaunay(tree::DelaunayTree; n_dims::Int=3)
    for i in keys(tree.simplices)
        if !tree.simplices[i].dead
            for j in keys(tree.vertices)
                if j > 0
                    if in_sphere(i, tree.vertices[j], tree, n_dims=n_dims) && j ∉ tree.simplices[i].vertices
                        println("Error, point ", j, " is inside the circumcircle of simplex ", i)
                    end
                end
            end
        end
    end
end

check_delaunay(tree, n_dims=2)