class Rtop < Formula
  desc "A modern TUI system resource monitor with Docker and disk I/O tracking"
  homepage "https://github.com/rebienkrdns/rtop"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/rebienkrdns/rtop/releases/download/v0.1.0/rtop-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Placeholder, update with actual sha256 on release
    elsif Hardware::CPU.arm?
      url "https://github.com/rebienkrdns/rtop/releases/download/v0.1.0/rtop-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Placeholder, update with actual sha256 on release
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/rebienkrdns/rtop/releases/download/v0.1.0/rtop-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Placeholder, update with actual sha256 on release
    elsif Hardware::CPU.arm?
      url "https://github.com/rebienkrdns/rtop/releases/download/v0.1.0/rtop-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000" # Placeholder, update with actual sha256 on release
    end
  end

  def install
    bin.install "rtop"
  end

  test do
    assert_match "rtop", shell_output("#{bin}/rtop --help")
  end
end
