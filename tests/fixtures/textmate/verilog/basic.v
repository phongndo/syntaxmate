`timescale 1ns/1ps
`define RESET_VALUE 8'h00
module unicode_counter #(parameter WIDTH = 8) (
    input wire clk,
    input wire reset_n,
    output reg [WIDTH-1:0] count
);
  // Unicode payload: café λ 東京 🚀 𝌆
  /* A block comment keeps lexical state
     across a physical line, then closes. */
  localparam [WIDTH-1:0] LIMIT = 8'hFF;
  wire at_limit = (count == LIMIT);
  initial begin
    count = `RESET_VALUE;
    $display("boot café λ 東京 🚀 𝌆: %0d", count);
  end
  always @(posedge clk or negedge reset_n) begin
    if (!reset_n)
      count <= #1 `RESET_VALUE;
    else if (at_limit)
      count <= 0;
    else
      count <= count + 1'b1;
  end
endmodule
