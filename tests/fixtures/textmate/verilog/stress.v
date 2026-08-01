`timescale 1ns/10ps
`default_nettype none
`define DATA_WIDTH 16
`define RESET_WORD 16'hCAFE
/* Stress fixture for the classic Verilog grammar.
   Unicode remains in comments and strings: café λ 東京 🚀 𝌆.
   This deliberately spans lines before closing the comment. */
module arithmetic_unit #(
    parameter WIDTH = `DATA_WIDTH,
    parameter RESET_VALUE = `RESET_WORD
) (
    input wire clk,
    input wire reset_n,
    input wire enable,
    input wire [2:0] opcode,
    input wire signed [WIDTH-1:0] lhs,
    input wire signed [WIDTH-1:0] rhs,
    output reg signed [WIDTH-1:0] result,
    output reg carry,
    output wire ready
);
  localparam OP_ADD = 3'd0;
  localparam OP_SUB = 3'd1;
  localparam OP_AND = 3'd2;
  localparam OP_OR  = 3'd3;
  localparam OP_XOR = 3'd4;
  localparam OP_SHL = 3'd5;
  localparam OP_SHR = 3'd6;
  localparam OP_MUL = 3'd7;
  integer cycle_count;
  real scale_factor;
  realtime last_edge;
  time timeout_value;
  reg [WIDTH:0] wide_result;
  reg [7:0] memory [0:15];
  wire eq_flag;
  wire neq_flag;
  wire less_flag;
  wire logic_flag;
  tri shared_bus;
  wand all_drivers;
  wor any_driver;
  supply1 power_net;
  supply0 ground_net;
  assign eq_flag = (lhs == rhs);
  assign neq_flag = (lhs != rhs);
  assign less_flag = (lhs < rhs);
  assign logic_flag = enable && ready || !reset_n;
  assign ready = reset_n & ~carry;
  assign shared_bus = enable ? lhs[0] : 1'bz;
  assign all_drivers = lhs[0];
  assign all_drivers = rhs[0];
  assign any_driver = lhs[1];
  assign any_driver = rhs[1];
  initial begin
    cycle_count = 0;
    scale_factor = 1.25e2;
    timeout_value = 1000;
    result = RESET_VALUE;
    carry = 1'b0;
    wide_result = {1'b0, RESET_VALUE};
    memory[0] = 8'b1010_0101;
    memory[1] = 8'o17;
    memory[2] = 8'd42;
    memory[3] = 8'hxF;
    $display("start café λ 東京 🚀 𝌆\nwidth=%0d", WIDTH);
    $timeformat(-9, 2, " ns", 12);
  end
  always @(posedge clk or negedge reset_n) begin
    if (!reset_n) begin
      result <= RESET_VALUE;
      carry <= 1'b0;
      cycle_count <= 0;
      last_edge <= $realtime;
    end else if (enable) begin
      cycle_count <= cycle_count + 1;
      case (opcode)
        OP_ADD: begin
          wide_result = lhs + rhs;
          result <= wide_result[WIDTH-1:0];
          carry <= wide_result[WIDTH];
        end
        OP_SUB: begin
          result <= lhs - rhs;
          carry <= lhs < rhs;
        end
        OP_AND: result <= lhs & rhs;
        OP_OR:  result <= lhs | rhs;
        OP_XOR: result <= lhs ^ rhs;
        OP_SHL: result <= lhs << rhs[3:0];
        OP_SHR: result <= lhs >> rhs[3:0];
        OP_MUL: result <= lhs * rhs;
        default: result <= {WIDTH{1'bx}};
      endcase
    end
  end
  always @(negedge clk) begin
    if (cycle_count % 4 == 0)
      $write("cycle=%0d result=%h", cycle_count, result);
    else
      $display(" pending");
  end
  function [WIDTH-1:0] rotate_left;
    input [WIDTH-1:0] value;
    input integer amount;
    begin
      rotate_left = (value << amount) | (value >> (WIDTH-amount));
    end
  endfunction
  task clear_memory;
    integer index;
    begin
      for (index = 0; index < 16; index = index + 1) begin
        memory[index] = 8'h00;
      end
    end
  endtask
  generate
    genvar bit_index;
    for (bit_index = 0; bit_index < WIDTH; bit_index = bit_index + 1) begin : parity_bits
      wire folded;
      assign folded = lhs[bit_index] ^ rhs[bit_index];
    end
  endgenerate
  specify
    specparam rise_delay = 2, fall_delay = 3;
    (clk *> result) = (rise_delay, fall_delay);
    $setuphold(posedge clk, posedge enable, 1, 1);
  endspecify
endmodule

module leaf_cell (
    input wire a,
    input wire b,
    output wire y
);
  xor primitive_xor(y, a, b);
endmodule

module integration_top;
  parameter TOP_WIDTH = 16;
  reg clk;
  reg reset_n;
  reg enable;
  reg [2:0] opcode;
  reg signed [TOP_WIDTH-1:0] lhs;
  reg signed [TOP_WIDTH-1:0] rhs;
  wire signed [TOP_WIDTH-1:0] result;
  wire carry;
  wire ready;
  wire leaf_y;
  arithmetic_unit #(
      .WIDTH(TOP_WIDTH),
      .RESET_VALUE(16'h1234)
  ) dut (
      .clk(clk),
      .reset_n(reset_n),
      .enable(enable),
      .opcode(opcode),
      .lhs(lhs),
      .rhs(rhs),
      .result(result),
      .carry(carry),
      .ready(ready)
  );
  leaf_cell leaf_instance (
      .a(lhs[0]),
      .b(rhs[0]),
      .y(leaf_y)
  );
  initial begin
    clk = 0;
    reset_n = 0;
    enable = 0;
    opcode = 0;
    lhs = -16'sd3;
    rhs = 16'd7;
    #5 reset_n = 1;
    #5 enable = 1;
    repeat (8) begin
      #10 opcode = opcode + 1'b1;
    end
    fork
      #2 lhs = 16'h00FF;
      #3 rhs = 16'h0F0F;
    join
    wait (ready === 1'b1);
    if (leaf_y !== 1'bx)
      $display("leaf=%b", leaf_y);
    $finish;
  end
  always #5 clk = ~clk;
endmodule

primitive udp_mux (out, select, first, second);
  output out;
  input select, first, second;
  table
    0 0 ? : 0;
    0 1 ? : 1;
    1 ? 0 : 0;
    1 ? 1 : 1;
    x 0 0 : 0;
    x 1 1 : 1;
  endtable
endprimitive
`undef RESET_WORD
`undef DATA_WIDTH
`default_nettype wire
