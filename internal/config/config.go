package config

import (
	"fmt"
	"os"

	"gopkg.in/yaml.v3"
)

type SkillTarget struct {
	Name string `yaml:"name"`
	Path string `yaml:"path,omitempty"`
}

type SkillsField struct {
	Wildcard bool
	Items    []SkillTarget
}

func (s *SkillsField) UnmarshalYAML(node *yaml.Node) error {
	if node.Kind == yaml.ScalarNode && node.Value == "*" {
		s.Wildcard = true
		return nil
	}
	if node.Kind != yaml.SequenceNode {
		return fmt.Errorf("skills must be '*' or list")
	}
	for _, it := range node.Content {
		switch it.Kind {
		case yaml.ScalarNode:
			s.Items = append(s.Items, SkillTarget{Name: it.Value})
		case yaml.MappingNode:
			var st SkillTarget
			if err := it.Decode(&st); err != nil {
				return err
			}
			if st.Name == "" {
				return fmt.Errorf("skill mapping requires name")
			}
			s.Items = append(s.Items, st)
		default:
			return fmt.Errorf("invalid skill entry")
		}
	}
	return nil
}

type SourceSpec struct {
	Source string      `yaml:"source"`
	Branch string      `yaml:"branch,omitempty"`
	Skills SkillsField `yaml:"skills"`
}

type Config struct {
	Destination string       `yaml:"destination"`
	Sources     []SourceSpec `yaml:"skills"`
}

func Load(path string) (Config, error) {
	var cfg Config
	b, err := os.ReadFile(path)
	if err != nil {
		return cfg, err
	}
	if err := yaml.Unmarshal(b, &cfg); err != nil {
		return cfg, err
	}
	if cfg.Destination == "" {
		return cfg, fmt.Errorf("destination is required")
	}
	if len(cfg.Sources) == 0 {
		return cfg, fmt.Errorf("skills sources are required")
	}
	for _, s := range cfg.Sources {
		if s.Source == "" {
			return cfg, fmt.Errorf("source is required")
		}
	}
	return cfg, nil
}
