`timescale 1ns/1ps
package unicode_pkg;
  typedef enum logic [1:0] {IDLE = 2'b00, RUN = 2'b01, DONE = 2'b10} state_t;
  typedef struct packed { logic [7:0] data; bit valid; } packet_t;
  localparam string LABEL = "café Ω 😀";
  function automatic int add(input int lhs, input int rhs);
    return lhs + rhs;
  endfunction
endpackage

module counter #(parameter int WIDTH = 8) (
  input logic clk, rst_n, enable, output logic done
);
  import unicode_pkg::*;
  logic [WIDTH-1:0] count;
  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) count <= '0; else if (enable) count <= count + 1'b1;
  end
  assign done = &count;
  property eventually_done; @(posedge clk) enable |-> ##[1:WIDTH] done; endproperty
  done_a: assert property (eventually_done) else $error("count=%0d", count);
endmodule
