// Extended cases from yaml-test-suite data tag 2022-01-17.

macro_rules! json_case {
    ($name:ident, $yaml:literal, $json:literal) => {
        #[test]
        fn $name() {
            super::yaml_suite_support::assert_json_case($yaml, $json);
        }
    };
}

json_case!(
    yaml_suite_26dv,
    "\"top1\" : \n  \"key1\" : &alias1 scalar1\n'top2' : \n  'key2' : &alias2 scalar2\ntop3: &node3 \n  *alias1 : scalar3\ntop4: \n  *alias2 : scalar4\ntop5   :    \n  scalar5\ntop6: \n  &anchor6 'key6' : scalar6\n",
    "{\n  \"top1\": {\n    \"key1\": \"scalar1\"\n  },\n  \"top2\": {\n    \"key2\": \"scalar2\"\n  },\n  \"top3\": {\n    \"scalar1\": \"scalar3\"\n  },\n  \"top4\": {\n    \"scalar2\": \"scalar4\"\n  },\n  \"top5\": \"scalar5\",\n  \"top6\": {\n    \"key6\": \"scalar6\"\n  }\n}\n"
);

json_case!(yaml_suite_27na, "%YAML 1.2\n--- text\n", "\"text\"\n");

json_case!(
    yaml_suite_35kp,
    "--- !!map\n? a\n: b\n--- !!seq\n- !!str c\n--- !!str\nd\ne\n",
    "{\n  \"a\": \"b\"\n}\n[\n  \"c\"\n]\n\"d e\"\n"
);

json_case!(
    yaml_suite_36f6,
    "---\nplain: a\n b\n\n c\n",
    "{\n  \"plain\": \"a b\\nc\"\n}\n"
);

json_case!(yaml_suite_52dl, "---\n! a\n", "\"a\"\n");

json_case!(
    yaml_suite_54t7,
    "{foo: you, bar: far}\n",
    "{\n  \"foo\": \"you\",\n  \"bar\": \"far\"\n}\n"
);

json_case!(
    yaml_suite_57h4,
    "sequence: !!seq\n- entry\n- !!seq\n - nested\nmapping: !!map\n foo: bar\n",
    "{\n  \"sequence\": [\n    \"entry\",\n    [\n      \"nested\"\n    ]\n  ],\n  \"mapping\": {\n    \"foo\": \"bar\"\n  }\n}\n"
);

json_case!(yaml_suite_58mp, "{x: :x}\n", "{\n  \"x\": \":x\"\n}\n");

json_case!(
    yaml_suite_5we3,
    "? explicit key # Empty value\n? |\n  block key\n: - one # Explicit compact\n  - two # block value\n",
    "{\n  \"explicit key\": null,\n  \"block key\\n\": [\n    \"one\",\n    \"two\"\n  ]\n}\n"
);

json_case!(yaml_suite_65wh, "- foo\n", "[\n  \"foo\"\n]\n");

json_case!(
    yaml_suite_7w2p,
    "? a\n? b\nc:\n",
    "{\n  \"a\": null,\n  \"b\": null,\n  \"c\": null\n}\n"
);

json_case!(
    yaml_suite_7z25,
    "---\nscalar1\n...\nkey: value\n",
    "\"scalar1\"\n{\n  \"key\": \"value\"\n}\n"
);

json_case!(
    yaml_suite_7zz5,
    "---\nnested sequences:\n- - - []\n- - - {}\nkey1: []\nkey2: {}\n",
    "{\n  \"nested sequences\": [\n    [\n      [\n        []\n      ]\n    ],\n    [\n      [\n        {}\n      ]\n    ]\n  ],\n  \"key1\": [],\n  \"key2\": {}\n}\n"
);

json_case!(yaml_suite_82an, "---word1\nword2\n", "\"---word1 word2\"\n");

json_case!(
    yaml_suite_87e4,
    "'implicit block key' : [\n  'implicit flow key' : value,\n ]\n",
    "{\n  \"implicit block key\": [\n    {\n      \"implicit flow key\": \"value\"\n    }\n  ]\n}\n"
);

json_case!(
    yaml_suite_8cwc,
    "---\nkey ends with two colons::: value\n",
    "{\n  \"key ends with two colons::\": \"value\"\n}\n"
);

json_case!(yaml_suite_8g76, "  # Comment\n   \n\n\n", "");

json_case!(
    yaml_suite_8kb6,
    "---\n- { single line, a: b}\n- { multi\n  line, a: b}\n",
    "[\n  {\n    \"single line\": null,\n    \"a\": \"b\"\n  },\n  {\n    \"multi line\": null,\n    \"a\": \"b\"\n  }\n]\n"
);

json_case!(yaml_suite_8mk2, "! a\n", "\"a\"\n");

json_case!(
    yaml_suite_8qbe,
    "key:\n - item1\n - item2\n",
    "{\n  \"key\": [\n    \"item1\",\n    \"item2\"\n  ]\n}\n"
);

json_case!(
    yaml_suite_8udb,
    "[\n\"double\n quoted\", 'single\n           quoted',\nplain\n text, [ nested ],\nsingle: pair,\n]\n",
    "[\n  \"double quoted\",\n  \"single quoted\",\n  \"plain text\",\n  [\n    \"nested\"\n  ],\n  {\n    \"single\": \"pair\"\n  }\n]\n"
);

json_case!(
    yaml_suite_8xyn,
    "---\n- &\u{1f601} unicode anchor\n",
    "[\n  \"unicode anchor\"\n]\n"
);

json_case!(
    yaml_suite_93jh,
    " - key: value\n   key2: value2\n -\n   key3: value3\n",
    "[\n  {\n    \"key\": \"value\",\n    \"key2\": \"value2\"\n  },\n  {\n    \"key3\": \"value3\"\n  }\n]\n"
);

json_case!(
    yaml_suite_93wf,
    "--- >-\n  trimmed\n  \n \n\n  as\n  space\n",
    "\"trimmed\\n\\n\\nas space\"\n"
);

json_case!(
    yaml_suite_96l6,
    "--- >\n  Mark McGwire's\n  year was crippled\n  by a knee injury.\n",
    "\"Mark McGwire's year was crippled by a knee injury.\\n\"\n"
);

json_case!(
    yaml_suite_96nn_00,
    "foo: |-\n \tbar\n",
    "{\"foo\":\"\\tbar\"}\n"
);

json_case!(
    yaml_suite_96nn_01,
    "foo: |-\n \tbar",
    "{\"foo\":\"\\tbar\"}\n"
);

json_case!(yaml_suite_98yd, "# Comment only.\n", "");

json_case!(
    yaml_suite_9j7a,
    "foo:\n  bar: baz\n",
    "{\n  \"foo\": {\n    \"bar\": \"baz\"\n  }\n}\n"
);

json_case!(
    yaml_suite_9u5k,
    "---\n# Products purchased\n- item    : Super Hoop\n  quantity: 1\n- item    : Basketball\n  quantity: 4\n- item    : Big Shoes\n  quantity: 1\n",
    "[\n  {\n    \"item\": \"Super Hoop\",\n    \"quantity\": 1\n  },\n  {\n    \"item\": \"Basketball\",\n    \"quantity\": 4\n  },\n  {\n    \"item\": \"Big Shoes\",\n    \"quantity\": 1\n  }\n]\n"
);

json_case!(
    yaml_suite_9wxw,
    "# Private\n!foo \"bar\"\n...\n# Global\n%TAG ! tag:example.com,2000:app/\n---\n!foo \"bar\"\n",
    "\"bar\"\n\"bar\"\n"
);

json_case!(
    yaml_suite_9yrd,
    "a\nb  \n  c\nd\n\ne\n",
    "\"a b c d\\ne\"\n"
);

json_case!(
    yaml_suite_cup7,
    "anchored: !local &anchor value\nalias: *anchor\n",
    "{\n  \"anchored\": \"value\",\n  \"alias\": \"value\"\n}\n"
);

json_case!(
    yaml_suite_d83l,
    "- |2-\n  explicit indent and chomp\n- |-2\n  chomp and explicit indent\n",
    "[\n  \"explicit indent and chomp\",\n  \"chomp and explicit indent\"\n]\n"
);

json_case!(
    yaml_suite_d88j,
    "a: [b, c]\n",
    "{\n  \"a\": [\n    \"b\",\n    \"c\"\n  ]\n}\n"
);

json_case!(yaml_suite_d9tu, "foo: bar\n", "{\n  \"foo\": \"bar\"\n}\n");

json_case!(
    yaml_suite_dbg4,
    "# Outside flow collection:\n- ::vector\n- \": - ()\"\n- Up, up, and away!\n- -123\n- http://example.com/foo#bar\n# Inside flow collection:\n- [ ::vector,\n  \": - ()\",\n  \"Up, up and away!\",\n  -123,\n  http://example.com/foo#bar ]\n",
    "[\n  \"::vector\",\n  \": - ()\",\n  \"Up, up, and away!\",\n  -123,\n  \"http://example.com/foo#bar\",\n  [\n    \"::vector\",\n    \": - ()\",\n    \"Up, up and away!\",\n    -123,\n    \"http://example.com/foo#bar\"\n  ]\n]\n"
);

json_case!(
    yaml_suite_dc7x,
    "a: b\t\nseq:\t\n - a\t\nc: d\t#X\n",
    "{\n  \"a\": \"b\",\n  \"seq\": [\n    \"a\"\n  ],\n  \"c\": \"d\"\n}\n"
);

json_case!(
    yaml_suite_de56_00,
    "\"1 trailing\\t\n    tab\"\n",
    "\"1 trailing\\t tab\"\n"
);

json_case!(
    yaml_suite_de56_01,
    "\"2 trailing\\t  \n    tab\"\n",
    "\"2 trailing\\t tab\"\n"
);

json_case!(
    yaml_suite_de56_02,
    "\"3 trailing\\\t\n    tab\"\n",
    "\"3 trailing\\t tab\"\n"
);

json_case!(
    yaml_suite_de56_03,
    "\"4 trailing\\\t  \n    tab\"\n",
    "\"4 trailing\\t tab\"\n"
);

json_case!(
    yaml_suite_de56_04,
    "\"5 trailing\t\n    tab\"\n",
    "\"5 trailing tab\"\n"
);

json_case!(
    yaml_suite_de56_05,
    "\"6 trailing\t  \n    tab\"\n",
    "\"6 trailing tab\"\n"
);
