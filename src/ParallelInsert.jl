function batch_locate!(output::AbstractArray,vertices::AbstractArray, tree::DelaunayTree)
    for i in 1:length(vertices)
        output[i] = locate(Vector{Int}(), vertices[i], tree)
    end
    output
end

function parallel_locate!(vertices::Vector{Vector{Float64}}, tree::DelaunayTree)::Vector{Vector{Int}}
    output = Vector{Vector{Int}}(undef, length(vertices))
    batch_size = length(vertices) ÷ Threads.nthreads()
    Threads.@threads for i in 1:Threads.nthreads()
        if i == Threads.nthreads()
            slice = @view output[(i-1)*batch_size+1:end]
            batch_locate!(slice, vertices[(i-1)*batch_size+1:end], tree)
        else
            slice = @view output[(i-1)*batch_size+1:i*batch_size]
            batch_locate!(slice, vertices[(i-1)*batch_size+1:i*batch_size], tree)
        end
    end
    return output
end

function identify_nonconflict_points(vertices::Vector{Vector{Float64}}, tree::DelaunayTree)::Tuple{Vector{Vector{Int}},Vector{Int}}
    site_list = parallel_locate(vertices, tree)
    length_site_list = length(site_list)
    output = Vector{Bool}(undef, length_site_list)
    Threads.@threads for i in 1:length_site_list-1
        # output[i] = all(isempty.(map(x->intersect(reduce(vcat,tree.neighbors_relation[site_list[i]]), reduce(vcat, tree.neighbors_relation[site_list[x]])), i+1:length_site_list)))
        output[i] = all(isempty.(map(x->intersect(site_list[i], site_list[x]), i+1:length_site_list)))
    end
    return site_list, output
end

function batch_insert_point(vertices::Vector{Vector{Float64}}, tree::DelaunayTree)
    nonconflict_points = identify_nonconflict_points(vertices, tree)
    for i in 1:length(vertices)
        if nonconflict_points[i]
            insert_point(tree, vertices[i])
        end
    end
end

export parallel_locate!, batch_locate!, identify_nonconflict_points