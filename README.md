```
cargo run --release -- dependency-graph ../../src/Common --dot out.dot --json out.json;
dot -Tsvg out.dot -o out.svg
```

minidom doesn't work for csproj files because it doesn't take doctype and comments into account and it requires all elements to declare a namespace.

[`csprojtool mv` demo video](https://www.youtube.com/watch?v=3np3LUaPwgA)
