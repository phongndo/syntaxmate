library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity unicode_counter is
  generic (WIDTH : positive := 8);
  port (
    clk, reset_n : in std_logic;
    count : out unsigned(WIDTH - 1 downto 0)
  );
end entity unicode_counter;
architecture rtl of unicode_counter is
  signal value : unsigned(WIDTH - 1 downto 0) := (others => '0');
  constant banner : string := "café λ 東京 🚀 𝌆";
begin
  -- Unicode comment: café λ 東京 🚀 𝌆
  count <= value;
  clocked : process (clk, reset_n)
  begin
    if reset_n = '0' then
      value <= (others => '0');
    elsif rising_edge(clk) then
      value <= value + 1;
    end if;
  end process clocked;
end architecture rtl;
