###
Basic CoffeeScript fixture — café 東京 🚀 𝌆
@fixture grammar coverage
###
square = (n = 2) -> n * n
bound = (name) => "Hello, #{name}! λ"

class Greeter extends BaseGreeter
  constructor: (@name, @ready = yes) ->
  label: -> if @ready then bound(@name) else 'paused'

{name, score} = user
[first, rest...] = [1, 2, 3]
range = [0..score]
slice = range[1...3]
pattern = /// ^ (café|東京) \s+ #{score} $ ///g
plain = /rocket🚀/i
single = '''literal \n text'''
double = """interpolated #{name} 🚀"""
script = ``
view = <Card title="café">{square score}</Card>
console.log view, first, rest
