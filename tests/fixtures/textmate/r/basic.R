# A compact R sample: café, 東京, and an astral owl 🦉.
#' Summarise a numeric vector.
#' @param x Values to inspect.
#' @param ... Extra arguments passed to `mean()`.
summarise_values <- function(x = c(1L, 2.5, NA_real_), ...) {
  clean <- x[!is.na(x)]
  if (length(clean) == 0L) {
    return(NULL)
  }
  list(mean = mean(clean, ...), range = range(clean))
}

message_text <- "λ says \"hello\" to 🙂\n"
records <- data.frame(id = 1:3, label = c("alpha", "βeta", "gamma"))
records$score <- c(Inf, 0x10L, 3e-2)
selected <- records[records$id %in% c(1L, 3L), c("label", "score")]
model_formula <- score ~ id + I(id^2)
for (name in selected$label) {
  print(paste(message_text, name))
}
