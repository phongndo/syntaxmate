`timescale 1ns/1ps
`default_nettype none
`define RESET_VALUE(W) {W{1'b0}}
`ifdef FORMAL
  `define CHECK_KIND assume
`else
  `define CHECK_KIND assert
`endif

package bus_pkg;
  timeunit 1ns;
  timeprecision 1ps;
  parameter int unsigned DATA_W = 32;
  localparam time SETUP = 250ps;
  typedef enum logic [2:0] {
    OP_IDLE = 3'd0,
    OP_READ = 3'd1,
    OP_WRITE = 3'd2,
    OP_ERROR = 3'd7
  } opcode_e;
  typedef struct packed {
    opcode_e opcode;
    logic [7:0] tag;
    logic [DATA_W-1:0] payload;
  } request_t;
  typedef union packed {
    logic [DATA_W-1:0] word;
    byte lanes[DATA_W/8];
  } word_u;
  typedef logic [DATA_W-1:0] data_t;
  const string BANNER = "naïve λ monitor 🚀";

  function automatic int unsigned parity(input data_t value);
    return ^value;
  endfunction

  task automatic report(input string prefix, input request_t req);
    $display("%s tag=%0d data=%08h", prefix, req.tag, req.payload);
  endtask
endpackage : bus_pkg

interface bus_if #(parameter int WIDTH = bus_pkg::DATA_W) (input logic clk);
  logic rst_n;
  logic valid;
  logic ready;
  logic [WIDTH-1:0] data;
  clocking cb @(posedge clk);
    default input #1step output #1ns;
    output valid, data;
    input ready;
  endclocking
  modport master(clocking cb, output rst_n);
  modport slave(input clk, rst_n, valid, data, output ready);
endinterface : bus_if

(* keep_hierarchy = "yes", purpose = "Unicode Ω 😀" *)
module fifo #(
  parameter int WIDTH = 32,
  parameter int DEPTH = 8,
  parameter bit FALL_THROUGH = 1'b0
) (
  input logic clk,
  input logic rst_n,
  input logic push,
  input logic pop,
  input logic [WIDTH-1:0] write_data,
  output logic [WIDTH-1:0] read_data,
  output logic full,
  output logic empty
);
  localparam int ADDR_W = $clog2(DEPTH);
  logic [WIDTH-1:0] memory [0:DEPTH-1];
  logic [ADDR_W:0] used;
  logic [ADDR_W-1:0] write_ptr, read_ptr;
  wire accepting = push && !full;
  wire producing = pop && !empty;

  assign full = (used == DEPTH);
  assign empty = (used == 0);
  assign read_data = memory[read_ptr];

  always_ff @(posedge clk or negedge rst_n) begin : pointers
    if (!rst_n) begin
      used <= '0;
      write_ptr <= `RESET_VALUE(ADDR_W);
      read_ptr <= '0;
    end else begin
      unique case ({accepting, producing})
        2'b10: used <= used + 1'b1;
        2'b01: used <= used - 1'b1;
        default: used <= used;
      endcase
      if (accepting) begin
        memory[write_ptr] <= write_data;
        write_ptr <= (write_ptr == DEPTH-1) ? '0 : write_ptr + 1'b1;
      end
      if (producing)
        read_ptr <= (read_ptr == DEPTH-1) ? '0 : read_ptr + 1'b1;
    end
  end

  generate
    for (genvar lane = 0; lane < WIDTH/8; lane++) begin : lane_status
      logic byte_nonzero;
      always_comb byte_nonzero = |read_data[lane*8 +: 8];
    end
    if (FALL_THROUGH) begin : passthrough
      always_comb if (empty && push) read_data = write_data;
    end
  endgenerate

  sequence push_then_pop;
    accepting ##[1:DEPTH] producing;
  endsequence
  property occupancy_safe;
    @(posedge clk) disable iff (!rst_n) used inside {[0:DEPTH]};
  endproperty
  safe_a: assert property (occupancy_safe) else $fatal(1, "used=%0d", used);
  flow_c: cover property (@(posedge clk) push_then_pop);
endmodule : fifo

module controller(bus_if.slave bus);
  import bus_pkg::*;
  request_t current;
  opcode_e state, next_state;
  int unsigned retries = 0;
  real scale = 1.25e+2;
  realtime deadline = 10.5ns;
  logic signed [15:0] delta = -16'sd3;
  logic [31:0] masks [string];
  data_t queue[$];

  always_comb begin
    next_state = state;
    priority casez (state)
      OP_IDLE:  if (bus.valid) next_state = OP_READ;
      OP_READ:  next_state = bus.ready ? OP_IDLE : OP_ERROR;
      OP_WRITE: next_state = OP_IDLE;
      default:  next_state = OP_ERROR;
    endcase
  end

  always_ff @(posedge bus.clk or negedge bus.rst_n) begin
    if (!bus.rst_n) begin
      state <= OP_IDLE;
      retries <= 0;
    end else begin
      state <= next_state;
      if (state == OP_ERROR) retries++;
      current.payload <= bus.data;
    end
  end

  always_latch begin
    if (bus.valid) current.tag <= bus.data[7:0];
  end

  task automatic drive_idle(ref logic signal);
    signal = 1'b0;
    #1ns;
  endtask

  function automatic data_t transform(input data_t raw);
    data_t rotated;
    rotated = {raw[15:0], raw[31:16]};
    return data_t'(rotated ^ 32'hA5A5_F00D);
  endfunction

  initial begin : stimulus
    automatic int seed = 32'hC0FF_EE00;
    void'($urandom(seed));
    masks["low"] = 32'o0000_0377;
    masks["high"] = 32'b1111_0000_xxxx_zzzz;
    queue = '{32'd1, 32'd2, 32'd3};
    fork
      begin repeat (3) @(posedge bus.clk); end
      begin wait (bus.rst_n === 1'b1); end
    join_any
    disable fork;
    foreach (queue[i]) queue[i] <<= 1;
  end
endmodule : controller

class transaction #(type T = bus_pkg::request_t);
  rand bit [7:0] delay;
  randc bus_pkg::opcode_e kind;
  rand T item;
  protected string note = "café transaction 🧪";
  constraint legal_delay { delay inside {[1:20]}; delay dist {1 := 4, [2:10] :/ 8}; }
  constraint ordered { solve kind before delay; }

  function new(string note = "default");
    this.note = note;
  endfunction

  virtual function string sprint();
    return $sformatf("kind=%s delay=%0d", kind.name(), delay);
  endfunction
endclass : transaction

covergroup traffic_cg @(posedge top.clk);
  option.per_instance = 1;
  kind_cp: coverpoint top.ctrl.state {
    bins normal[] = {bus_pkg::OP_IDLE, bus_pkg::OP_READ, bus_pkg::OP_WRITE};
    illegal_bins bad = {bus_pkg::OP_ERROR};
  }
  retry_cp: coverpoint top.ctrl.retries { bins few = {[0:3]}; }
  kind_x_retry: cross kind_cp, retry_cp;
endgroup : traffic_cg

module top;
  logic clk = 1'b0;
  always #5ns clk = ~clk;
  bus_if #(.WIDTH(32)) bus(clk);
  fifo #(.WIDTH(32), .DEPTH(8)) dut (
    .clk, .rst_n(bus.rst_n), .push(bus.valid), .pop(bus.ready),
    .write_data(bus.data), .read_data(), .full(), .empty()
  );
  controller ctrl(.bus(bus));
  initial begin
    bus.rst_n = 1'b0;
    bus.valid = 1'b0;
    bus.data = '0;
    #12ns bus.rst_n = 1'b1;
  end
endmodule : top

bind fifo fifo_checker #(.LIMIT(DEPTH)) checks (.*);
module fifo_checker #(parameter int LIMIT = 8) (input logic clk, rst_n);
  default clocking mon_cb @(posedge clk); endclocking
  default disable iff (!rst_n);
  never_unknown: assert property (!$isunknown(rst_n));
endmodule : fifo_checker

primitive udp_mux(out, select, a, b);
  output out;
  input select, a, b;
  table
    0 0 ? : 0;
    0 1 ? : 1;
    1 ? 0 : 0;
    1 ? 1 : 1;
    x 0 0 : 0;
    x 1 1 : 1;
  endtable
endprimitive

`default_nettype wire
