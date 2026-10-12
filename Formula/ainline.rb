class Ainline < Formula
  desc "Run AI providers with reusable tasks and project context"
  homepage "https://github.com/PabloAlcoleaSesse/ainline"
  head "https://github.com/PabloAlcoleaSesse/ainline.git", branch: "main"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--path", ".", "--root", prefix
    generate_completions_from_executable(bin/"ai", "completions", shells: [:bash, :zsh, :fish])
  end

  test do
    assert_equal "hello\n", shell_output("#{bin}/ai --dry-run --no-context hello")
  end
end
