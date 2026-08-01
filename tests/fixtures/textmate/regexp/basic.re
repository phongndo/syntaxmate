(?# TODO: compact Python-regexp coverage with λ 東京 🚀 𝌆)
(?aimsx)
^(?:cat|dog).+?\s*$
\A\b[A-Z]\w*\B.*\Z
a{2}b{3,}c{1,4}d{,5}
\x41\077\0\u03BB\U0001F680
\d+\D*\s?\S+\w??\W\q
[a-zA-Z0-9_.-]+[^\s#][\a\b\f\n\r\t\v\\]
[]a-z][^]0-9][\123\x2D\u6771\U0001D306\N\-]
(?P<word>café|東京|λ+)-(?P=word)
([A-Z]{2})(\d{2})-\1-\2
(?=https?://)(?:https?|ftp)://[^\s/]+(?:/[^\s]*)?
(?!.*(?:TODO|FIXME))^[a-z_][a-z0-9_]*$
(?<=prefix:)\w+(?=;|$)
(?<!not-)allowed(?!-suffix)
(?P<sign>[+-])?\d+(?(sign)\.\d+|\.0)
(a)?b(?(1)c|d)
((?:nested|grouped)+)(?# HACK: exercise comment and parentheses)
[]|[^]|\.|\^|\$|\*|\+|\?|\|
^(?P<glyph>λ|東|🚀|𝌆){1,3}(?:\s+\u6771\U0001F680)?$
