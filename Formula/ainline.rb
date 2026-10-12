class Ainline < Formula
  desc "AI providers with project context"
  homepage "https://github.com/PabloAlcoleaSesse/ainline"
  version "1.0.0"
  license "MIT"
  on_macos do
    on_arm do
      url "https://github.com/PabloAlcoleaSesse/ainline/releases/download/v1.0.0/ai-aarch64-apple-darwin.tar.gz"
      sha256 "8193beca9206540611b8bacc5149ab09877bc986a60f75cf35264c9fade09362"
    end
    on_intel do
      url "https://github.com/PabloAlcoleaSesse/ainline/releases/download/v1.0.0/ai-x86_64-apple-darwin.tar.gz"
      sha256 "44bb30aaf963fde21069343943709d29beb23867470bde8ece39fc952a6f5189"
    end
  end
  on_linux do
    on_intel do
      url "https://github.com/PabloAlcoleaSesse/ainline/releases/download/v1.0.0/ai-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "098ac976ffdf6af3cf3ab7486ee91da70280304de5eeba64288d42ac8c898e60"
    end
  end
  def install
    bin.install "ai"
    generate_completions_from_executable(bin/"ai", "completions", shells: [:bash, :zsh, :fish])
  end
  test do
    assert_equal "hello\n", shell_output("#{bin}/ai --dry-run --no-context hello")
  end
end
