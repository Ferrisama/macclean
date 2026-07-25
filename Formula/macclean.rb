class Macclean < Formula
  desc "Mac system maintenance CLI -- clean, analyze, secure, monitor"
  homepage "https://github.com/Ferrisama/macclean"
  url "https://github.com/Ferrisama/macclean/archive/refs/tags/v0.3.0.tar.gz"
  # Update after tagging release:
  # scripts/update-homebrew-formula.sh 0.3.0
  sha256 "FILL_IN_AFTER_RELEASE_TAG"
  license "MIT"
  head "https://github.com/Ferrisama/macclean.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "macclean", shell_output("#{bin}/macclean --version")
  end
end
