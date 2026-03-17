class Sukiru < Formula
  desc "Fast binary CLI to sync AI skills from local and GitHub sources"
  homepage "https://github.com/pivoshenko/sukiru"
  url "https://github.com/pivoshenko/sukiru/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match "sukiru", shell_output("#{bin}/sukiru sync --help 2>&1", 0)
  end
end
