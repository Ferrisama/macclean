class Macclean < Formula
  include Language::Python::Virtualenv

  desc "Mac system maintenance CLI — clean, analyze, secure, monitor"
  homepage "https://github.com/Ferrisama/macclean"
  url "https://files.pythonhosted.org/packages/source/m/macclean/macclean-0.2.0.tar.gz"
  # Update sha256 after publishing to PyPI:
  # curl -sL <url> | shasum -a 256
  sha256 "FILL_IN_AFTER_PYPI_PUBLISH"
  license "MIT"

  depends_on "python@3.11"

  resource "click" do
    url "https://files.pythonhosted.org/packages/source/c/click/click-8.1.7.tar.gz"
    sha256 "ca9853ad459e787e2192211578cc907e7594e294c7ccc834310722b41b9ca6de"
  end

  resource "rich" do
    url "https://files.pythonhosted.org/packages/source/r/rich/rich-13.7.1.tar.gz"
    sha256 "9be308cb1fe2f1f57d67ce99e95af38a1e2bc71ad9813b0e247cf7ffbcc3a432"
  end

  resource "questionary" do
    url "https://files.pythonhosted.org/packages/source/q/questionary/questionary-2.0.1.tar.gz"
    sha256 "bcce898bf3dbb446ff62830c86c5c6fb9a22a54146f0f5597d3da43b1d7b8c59"
  end

  resource "psutil" do
    url "https://files.pythonhosted.org/packages/source/p/psutil/psutil-5.9.8.tar.gz"
    sha256 "6be126e3225486dff286a8fb9a06246a5253f4c7c53b475ea5f5ac934e64194c"
  end

  def install
    virtualenv_install_with_resources
  end

  test do
    system bin/"macclean", "--help"
  end
end
