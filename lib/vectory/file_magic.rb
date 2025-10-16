module Vectory
  class FileMagic
    EPS_30_MAGIC = "\x25\x21\x50\x53\x2d\x41\x64\x6f\x62\x65\x2d\x33\x2e\x30" \
                   "\x20\x45\x50\x53\x46\x2d\x33\x2e\x30"
      .force_encoding("BINARY") # "%!PS-Adobe-3.0 EPSF-3.0"

    EPS_31_MAGIC = "\x25\x21\x50\x53\x2d\x41\x64\x6f\x62\x65\x2d\x33\x2e\x31" \
                   "\x20\x45\x50\x53\x46\x2d\x33\x2e\x30"
      .force_encoding("BINARY") # "%!PS-Adobe-3.1 EPSF-3.0"

    PS_MAGIC = "\x25\x21\x50\x53\x2d\x41\x64\x6f\x62\x65\x2d\x33\x2e\x30"
      .force_encoding("BINARY") # "%!PS-Adobe-3.0"

    EMF_MAGIC = "\x01\x00\x00\x00".force_encoding("BINARY")

    def self.detect(path)
      new(path).detect
    end

    def initialize(path)
      @path = path
    end

    def detect
      beginning = File.read(@path, 100, mode: "rb")

      eps_slice = beginning.byteslice(0, EPS_30_MAGIC.size)
      Vectory.ui.debug("File magic is '#{to_bytes(beginning)}' of '#{@path}'.")

      return :eps if [EPS_30_MAGIC, EPS_31_MAGIC].include?(eps_slice)

      ps_slice = beginning.byteslice(0, PS_MAGIC.size)
      return :ps if ps_slice == PS_MAGIC

      emf_slice = beginning.byteslice(0, EMF_MAGIC.size)
      return :emf if emf_slice == EMF_MAGIC

      :svg if contain_svg_tag?
    end

    private

    def to_bytes(str)
      str.unpack("c*").map { |e| "\\x#{e.to_s(16).rjust(2, '0')}" }.join
    end

    def contain_svg_tag?
      content = File.read(@path, 4096)

      :svg if content.include?("<svg")
    end
  end
end
