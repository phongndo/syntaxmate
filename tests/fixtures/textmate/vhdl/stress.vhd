library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;
library std;
use std.textio.all;
-- Grammar stress fixture: café λ 東京 🚀 𝌆
package unicode_support is
  constant WORD_SIZE : positive := 16;
  constant PI_COPY : real := MATH_PI;
  subtype word_t is unsigned(WORD_SIZE - 1 downto 0);
  subtype index_t is natural range 0 to 15;
  type state_t is (idle, load, execute, finish);
  type sample_t is record
    valid : boolean;
    data : word_t;
    stamp : time;
  end record;
  type memory_t is array (index_t) of word_t;
  function parity(value : word_t) return std_logic;
  procedure clear(variable target : out memory_t);
  component worker is
    generic (WIDTH : positive := WORD_SIZE);
    port (
      clk : in std_logic;
      reset_n : in std_logic;
      operand : in unsigned(WIDTH - 1 downto 0);
      result : out unsigned(WIDTH - 1 downto 0)
    );
  end component worker;
end package unicode_support;

package body unicode_support is
  function parity(value : word_t) return std_logic is
    variable folded : std_logic := '0';
  begin
    for bit_index in value'range loop
      folded := folded xor value(bit_index);
    end loop;
    return folded;
  end function parity;

  procedure clear(variable target : out memory_t) is
  begin
    for item in target'range loop
      target(item) := (others => '0');
    end loop;
  end procedure clear;
end package body unicode_support;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.unicode_support.all;
entity worker is
  generic (
    WIDTH : positive := WORD_SIZE;
    LATENCY : natural := 2
  );
  port (
    clk : in std_logic;
    reset_n : in std_logic;
    operand : in unsigned(WIDTH - 1 downto 0);
    result : out unsigned(WIDTH - 1 downto 0)
  );
end entity worker;

architecture behavioral of worker is
  signal accumulator : unsigned(WIDTH - 1 downto 0) := (others => '0');
begin
  sequential : process (clk, reset_n)
    variable delay_count : natural := 0;
  begin
    if reset_n = '0' then
      accumulator <= (others => '0');
      delay_count := 0;
    elsif rising_edge(clk) then
      if delay_count < LATENCY then
        delay_count := delay_count + 1;
      else
        accumulator <= rotate_left(operand, 1) + 1;
        delay_count := 0;
      end if;
    end if;
  end process sequential;
  result <= accumulator;
end architecture behavioral;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.unicode_support.all;
entity grammar_stress is
  generic (
    WIDTH : positive := WORD_SIZE;
    ENABLE_TRACE : boolean := true
  );
  port (
    clk : in std_logic;
    reset_n : in std_logic;
    command : in std_logic_vector(2 downto 0);
    lhs : in unsigned(WIDTH - 1 downto 0);
    rhs : in unsigned(WIDTH - 1 downto 0);
    result : out unsigned(WIDTH - 1 downto 0);
    ready : out std_logic
  );
end entity grammar_stress;

architecture rtl of grammar_stress is
  constant TITLE : string := "café λ 東京 🚀 𝌆";
  constant HEX_VALUE : std_logic_vector(7 downto 0) := X"AF";
  constant OCT_VALUE : std_logic_vector(5 downto 0) := O"17";
  constant BIN_VALUE : std_logic_vector(3 downto 0) := B"10XZ";
  constant BASE_VALUE : integer := 16#CAFE#;
  constant REAL_VALUE : real := 6.25E+2;
  signal current_state : state_t := idle;
  signal next_state : state_t := idle;
  signal computed : unsigned(WIDTH - 1 downto 0) := (others => '0');
  signal worker_result : unsigned(WIDTH - 1 downto 0);
  signal event_count : natural := 0;
  signal flag : std_logic := 'U';
  signal bus_value : std_logic_vector(7 downto 0) := (others => 'Z');
  alias low_byte : std_logic_vector(7 downto 0) is bus_value;
  attribute keep : string;
  attribute keep of computed : signal is "true";

  function choose(left, right : unsigned; select_left : boolean) return unsigned is
  begin
    if select_left then
      return left;
    else
      return right;
    end if;
  end function choose;

  procedure announce(constant message : in string) is
    variable output_line : line;
  begin
    write(output_line, message);
    writeline(output, output_line);
  end procedure announce;

  component tiny_gate
    port (
      a, b : in std_logic;
      y : out std_logic
    );
  end component tiny_gate;
begin
  result <= computed;
  ready <= '1' when current_state = finish else '0';
  worker_instance : entity work.worker(behavioral)
    generic map (
      WIDTH => WIDTH,
      LATENCY => 3
    )
    port map (
      clk => clk,
      reset_n => reset_n,
      operand => lhs,
      result => worker_result
    );

  gate_instance : tiny_gate
    port map (
      a => lhs(0),
      b => rhs(0),
      y => flag
    );

  state_decode : process (current_state, command, lhs, rhs, worker_result)
  begin
    next_state <= current_state;
    computed <= (others => '0');
    case current_state is
      when idle =>
        if command /= "000" then
          next_state <= load;
        end if;
      when load =>
        computed <= choose(lhs, rhs, command(0) = '1');
        next_state <= execute;
      when execute =>
        case command is
          when "001" => computed <= lhs + rhs;
          when "010" => computed <= lhs - rhs;
          when "011" => computed <= lhs and rhs;
          when "100" => computed <= lhs or rhs;
          when "101" => computed <= lhs xor rhs;
          when "110" => computed <= shift_left(lhs, 1);
          when others => computed <= worker_result;
        end case;
        next_state <= finish;
      when finish =>
        computed <= resize(to_unsigned(BASE_VALUE, WIDTH), WIDTH);
        next_state <= idle;
    end case;
  end process state_decode;

  state_register : process (clk, reset_n)
  begin
    if reset_n = '0' then
      current_state <= idle;
      event_count <= 0;
    elsif rising_edge(clk) then
      current_state <= next_state after 2 ns;
      event_count <= event_count + 1;
      assert not is_x(bus_value)
        report "bus contains unknown café λ 東京 🚀 𝌆"
        severity warning;
    end if;
  end process state_register;

  trace_enabled : if ENABLE_TRACE generate
    trace_process : process (clk)
    begin
      if rising_edge(clk) then
        report TITLE & integer'image(event_count) severity note;
      end if;
    end process trace_process;
  end generate trace_enabled;

  lanes : for lane in 0 to 3 generate
    lane_block : block
      signal lane_value : unsigned(3 downto 0);
    begin
      lane_value <= computed((lane * 4) + 3 downto lane * 4);
    end block lane_block;
  end generate lanes;

  watchdog : process
    variable attempts : natural := 0;
  begin
    while attempts < 3 loop
      wait until rising_edge(clk);
      attempts := attempts + 1;
      next when reset_n = '0';
      exit when ready = '1';
    end loop;
    wait for 10 ns;
  end process watchdog;
end architecture rtl;

configuration grammar_stress_cfg of grammar_stress is
  for rtl
    for gate_instance : tiny_gate
      use entity work.tiny_gate(rtl);
    end for;
  end for;
end configuration grammar_stress_cfg;
