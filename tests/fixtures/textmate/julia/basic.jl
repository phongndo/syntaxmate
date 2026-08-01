module BasicFixture

# A compact Julia sample: café, 東京, λ, 🚀, and 𝌆.
const ORBIT = :aurora
const VALUES = [0x2a, 0b1010, 3.5e2, π]

"""Return a labeled sum for `$name`."""
function summarize(name::AbstractString, values=VALUES)
    #= The block comment is multiline.
       TODO: keep interpolation and delimiters balanced. =#
    total = sum(values)
    message = """Mission $(name)
crosses 東京 with λ = $(round(total; digits=2)) 🚀 𝌆."""
    return (label=ORBIT, total=total, message=message)
end

matrix = [1 2; 3 4]'
pattern = r"^(?:café|東京)-\d+$"i
println(summarize("aurora", matrix[begin:end]))

end # module BasicFixture
