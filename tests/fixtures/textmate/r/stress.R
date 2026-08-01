# TextMate stress fixture for R: naïve façade, Ελληνικά, 東京, and rocket 🚀.
# The code is intentionally broad and need not be executed as one program.

#' Construct a small widget
#'
#' **Widgets** retain a value and optional metadata. See [base::list()].
#' Unicode documentation remains text: résumé, 雪, and telescope 🔭.
#'
#' @param value The value to retain.
#' @param label A _display_ label.
#' @param ... Named metadata fields.
#' @return A `widget` object.
#' @examples
#' make_widget(1, "demo", colour = "blue")
#' make_widget(value = pi, label = "π")
#' @export
make_widget <- function(value, label = "widget", ...) {
  structure(list(value = value, label = label, meta = list(...)), class = "widget")
}

# Assignment forms and syntactic or quoted identifiers.
ordinary <- 1L
equal_style = 2
3.5 -> rightward
4L ->> global_result
outer_value <<- 5
table_like <- list()
table_like$count <- 6L
table_like[["label"]] <- "assigned"
`odd name` <- "backtick identifier"
`tick\`tock` <- TRUE
assign("dynamic_name", 7, inherits = FALSE)
environment_holder <- new.env(parent = emptyenv())
environment_holder$value <- ordinary

# Numeric forms and language constants.
decimal_values <- c(0, 42L, .5, 1., 6.022e23, 1e-09)
hex_values <- c(0x0, 0xFFL, 0x1.fp3, 0XCAFE)
complex_values <- c(1i, 2 + 3i, 4.5e2i)
special_values <- list(TRUE, FALSE, NULL, NA, NaN, Inf, -Inf)
typed_missing <- c(NA_integer_, NA_real_, NA_complex_, NA_character_)

# Quoted strings, escapes, and literal Unicode.
double_escapes <- "quote=\" slash=\\ tab=\t newline=\n bell=\a"
single_escapes <- 'apostrophe=\' octal=\101 hex=\x42'
unicode_escapes <- "lambda=\u03bb smile=\U0001F642 brace=\u{2603}"
unicode_literal <- "BMP: café λ 東京; astral: 🐘 🌌"
# A comment also carries BMP ñ and astral music 𝄞.

# Raw strings use each delimiter family recognized by the grammar.
raw_parentheses <- r"(slashes \ stay literal; "quotes" stay too)"
raw_braces <- R"{a{b} and [brackets] without escapes}"
raw_brackets <- r'[line one
line two has ) and } plus emoji 🧪]'
raw_custom <- R"--(a closing parenthesis ) alone is harmless)--"

# Vectors, factors, matrices, lists, and data frames.
sequence_a <- 1:10
sequence_b <- seq(from = -1, to = 1, length.out = 5L)
logical_mask <- c(TRUE, FALSE, NA)
named_vector <- c(alpha = 1, beta = 2, gamma = 3)
recycled <- rep(c("x", "y"), times = 3L, each = 2L)
category <- factor(c("low", "high", "low"), levels = c("low", "high"))
matrix_value <- matrix(1:9, nrow = 3L, byrow = TRUE)
array_value <- array(1:8, dim = c(2L, 2L, 2L))
nested_list <- list(numbers = named_vector, flags = logical_mask, child = list(ok = TRUE))
frame <- data.frame(id = 1:3, name = c("Ada", "Béla", "蔡"), active = c(TRUE, FALSE, TRUE))

# Indexing, member access, slots, namespaces, and calls.
first <- named_vector[1L]
without_second <- named_vector[-2L]
chosen <- named_vector[c(TRUE, FALSE, TRUE)]
scalar <- nested_list[["numbers"]][[2L]]
child_ok <- nested_list$child$ok
matrix_corner <- matrix_value[1L, 3L, drop = FALSE]
frame_rows <- frame[frame$active & frame$id >= 1L, c("id", "name")]
median_fun <- stats::median
hidden_helper <- utils:::.DollarNames
slot_value <- formal_object@value

# Formula syntax and expression-building tools.
linear_formula <- response ~ predictor + group
quadratic_formula <- response ~ predictor + I(predictor^2)
one_sided_formula <- ~ x:y + x * y
update_formula <- response ~ . - ignored
language_vector <- expression(x + y, sin(theta), z <- 10L)
quoted_call <- quote(mean(x, na.rm = TRUE))
substituted <- substitute(a + b, list(a = quote(alpha), b = 2L))
backquoted <- bquote(.(ordinary) + beta)
argument_list <- alist(x =, y = 2, ...)
manual_call <- call("sum", 1L, 2L, na.rm = TRUE)
evaluated <- eval(quote(ordinary + equal_style), envir = environment())

# Functions, defaults, ellipsis, closures, and shorthand lambdas.
accumulator <- function(start = 0, step = 1L) {
  total <- start
  function(value = step, ...) {
    total <<- total + value
    total
  }
}

invoke <- function(fun = identity, x = NULL, ..., simplify = FALSE) {
  result <- fun(x, ...)
  if (simplify) unlist(result) else result
}

squares <- lapply(1:5, \(x) x^2)
named_defaults <- function(alpha = 1, beta = alpha + 1, gamma = list()) alpha + beta
invisible_return <- function(x) {
  if (missing(x)) return(invisible(NULL))
  invisible(x)
}

# Operators: arithmetic, comparisons, logic, matrices, help, and infix forms.
arithmetic <- (1 + 2 - 3) * 4 / 5 ^ 2
integer_math <- 17 %/% 5 + 17 %% 5
comparison <- arithmetic >= 0 && arithmetic != Inf || is.na(arithmetic)
vector_logic <- (sequence_a > 2) & (sequence_a <= 8) | !is.finite(sequence_a)
matrix_product <- matrix_value %*% diag(3L)
membership <- c("a", "z") %in% letters
range_test <- ordinary > 0 & ordinary < 10
?mean
help(package = "stats")

`%between%` <- function(x, bounds) x >= bounds[[1L]] & x <= bounds[[2L]]
inside <- sequence_a %between% c(3L, 7L)
formatted <- "value" %paste% ordinary
joined <- paste("a", "b") %>% toupper()
transformed <- frame |> subset(active) |> transform(code = toupper(name))

# A data-table-shaped expression exercises := without requiring the package.
table_update <- quote(DT[id > 1L, c("score", "flag") := list(value * 2, TRUE)])

# Conditional branches and switch expressions.
classify <- function(x) {
  if (is.null(x)) {
    "missing"
  } else if (is.numeric(x) && length(x) == 1L) {
    if (x < 0) "negative" else if (x == 0) "zero" else "positive"
  } else {
    switch(typeof(x), character = "text", logical = "flag", "other")
  }
}

choice <- switch("second", first = 1L, second = 2L, third = 3L)
compact_if <- if (TRUE) "yes" else "no"

# Loops cover next, break, while, and repeat.
collected <- list()
for (i in seq_along(sequence_a)) {
  if (i %% 2L == 0L) next
  collected[[length(collected) + 1L]] <- sequence_a[[i]]
  if (length(collected) >= 3L) break
}

countdown <- 3L
while (countdown > 0L) {
  countdown <- countdown - 1L
}

attempt <- 0L
repeat {
  attempt <- attempt + 1L
  if (attempt >= 2L) break
}

# S3-style generic and methods, including a quoted indexing method name.
describe <- function(x, ...) UseMethod("describe")
describe.default <- function(x, ...) paste("object of type", typeof(x))
describe.widget <- function(x, verbose = FALSE, ...) {
  text <- paste0("<widget ", x$label, ">")
  if (verbose) paste(text, deparse(x$value)) else text
}
print.widget <- function(x, ...) {
  cat(describe(x, ...), "\n")
  invisible(x)
}
`[.widget` <- function(x, i, ...) {
  NextMethod("[")
}
as.data.frame.widget <- function(x, row.names = NULL, optional = FALSE, ...) {
  data.frame(label = x$label, value = I(list(x$value)), row.names = row.names)
}

# Conditions, handlers, cleanup, and restarts.
safe_log <- function(x) {
  tryCatch(
    {
      if (!is.numeric(x)) stop("x must be numeric", call. = FALSE)
      if (any(x < 0, na.rm = TRUE)) warning("negative input")
      log(x)
    },
    warning = function(w) {
      message("warning: ", conditionMessage(w))
      invokeRestart("muffleWarning")
    },
    error = function(e) structure(NA_real_, reason = conditionMessage(e)),
    finally = message("safe_log finished")
  )
}

possible <- try(sqrt(-1), silent = TRUE)
guarded <- withCallingHandlers(safe_log(c(1, -1)), message = function(m) invisible(NULL))
cleanup_demo <- function(path) {
  connection <- file(path, open = "r")
  on.exit(close(connection), add = TRUE)
  readLines(connection, warn = FALSE)
}

# Metaprogramming and miscellaneous plausible calls.
pairlist_value <- pairlist(alpha = 1, beta = quote(x + y))
dynamic_result <- do.call(sum, list(1L, 2L, 3L, na.rm = TRUE))
parsed <- parse(text = "x <- 1 + 2", keep.source = TRUE)
deparsed <- deparse1(quoted_call)
attributes(frame)$note <- "fixture metadata"
class(environment_holder) <- c("fixture_env", "environment")
rm(dynamic_name)

# Final balanced construct with Unicode at EOF nearby: Ω and satellite 🛰️.
final_result <- local({
  values <- Filter(is.finite, c(decimal_values, hex_values))
  Reduce(`+`, values, init = 0)
})
