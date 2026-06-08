class Macclean < Formula
  desc "Mac system maintenance CLI -- clean, analyze, secure, monitor"
  homepage "https://github.com/Ferrisama/macclean"
  url "https://github.com/Ferrisama/macclean/archive/refs/tags/v0.3.0.tar.gz"
  # Update sha256 after tagging release:
  # curl -sL <url> | shasum -a 256
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
