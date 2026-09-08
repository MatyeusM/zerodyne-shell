package performance

import "testing"

func TestCpuNameTrim(t *testing.T) {
  cases := map[string]string{
    "AMD Ryzen 5 7600 6-Core Processor": "AMD Ryzen 5 7600",
    "AMD Ryzen 9 7950X 16-Core Processor": "AMD Ryzen 9 7950X",
    "Intel(R) Core(TM) i7-14700K":        "Intel(R) Core(TM) i7-14700K",
    "Apple M1 Pro":                        "Apple M1 Pro",
  }
  for in, want := range cases {
    if got := cpuNameRe.ReplaceAllString(in, ""); got != want {
      t.Errorf("trim(%q) = %q, want %q", in, got, want)
    }
  }
}
