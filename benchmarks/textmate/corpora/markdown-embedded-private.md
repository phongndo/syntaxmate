# Private embedded grammar coverage

```yang
module demo {
  namespace "urn:demo";
  prefix demo;
}
```

```twig
{% if user %}{{ user.name }}{% endif %}
```

```clojure
(defn greet [name] (str "hello " name))
```

```latex
\section{Exact embedding}
```
