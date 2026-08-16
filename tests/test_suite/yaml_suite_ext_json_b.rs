// Extended yaml-test-suite cases with canonical JSON results.
//
// The input and expected result strings are copied byte-for-byte from the suite;
// non-ASCII characters are escaped so this source remains ASCII-only.

macro_rules! json_case {
    ($name:ident, $yaml:literal, $json:literal) => {
        #[test]
        fn $name() {
            super::yaml_suite_support::assert_json_case($yaml, $json);
        }
    };
}

// DHP8
json_case!(
    yaml_suite_dhp8,
    "[foo, bar, 42]\n",
    "[\n  \"foo\",\n  \"bar\",\n  42\n]\n"
);

// DK95/00
json_case!(
    yaml_suite_dk95_00,
    "foo:\n \tbar\n",
    "{\n  \"foo\" : \"bar\"\n}\n"
);

// DK95/02
json_case!(
    yaml_suite_dk95_02,
    "foo: \"bar\n  \tbaz\"\n",
    "{\n  \"foo\" : \"bar baz\"\n}\n"
);

// DK95/03
json_case!(yaml_suite_dk95_03, " \t\nfoo: 1\n", "{\n  \"foo\" : 1\n}\n");

// DK95/04
json_case!(
    yaml_suite_dk95_04,
    "foo: 1\n\t\nbar: 2\n",
    "{\n  \"foo\" : 1,\n  \"bar\" : 2\n}\n"
);

// DK95/05
json_case!(
    yaml_suite_dk95_05,
    "foo: 1\n \t\nbar: 2\n",
    "{\n  \"foo\" : 1,\n  \"bar\" : 2\n}\n"
);

// DK95/07
json_case!(yaml_suite_dk95_07, "%YAML 1.2\n\t\n---\n", "null\n");

// DK95/08
json_case!(
    yaml_suite_dk95_08,
    "foo: \"bar\n \t \t baz \t \t \"\n",
    "{\n  \"foo\" : \"bar baz \\t \\t \"\n}\n"
);

// DWX9
json_case!(
    yaml_suite_dwx9,
    "|\n \n  \n  literal\n   \n  \n  text\n\n # Comment\n",
    "\"\\n\\nliteral\\n \\n\\ntext\\n\"\n"
);

// E76Z
json_case!(
    yaml_suite_e76z,
    "&a a: &b b\n*b : *a\n",
    "{\n  \"a\": \"b\",\n  \"b\": \"a\"\n}\n"
);

// F3CP
json_case!(
    yaml_suite_f3cp,
    "---\n{ a: [b, c, { d: [e, f] } ] }\n",
    "{\n  \"a\": [\n    \"b\",\n    \"c\",\n    {\n      \"d\": [\n        \"e\",\n        \"f\"\n      ]\n    }\n  ]\n}\n"
);

// F6MC
json_case!(
    yaml_suite_f6mc,
    "---\na: >2\n   more indented\n  regular\nb: >2\n\n\n   more indented\n  regular\n",
    "{\n  \"a\": \" more indented\\nregular\\n\",\n  \"b\": \"\\n\\n more indented\\nregular\\n\"\n}\n"
);

// F8F9
json_case!(
    yaml_suite_f8f9,
    " # Strip\n  # Comments:\nstrip: |-\n  # text\n  \n # Clip\n  # comments:\n\nclip: |\n  # text\n \n # Keep\n  # comments:\n\nkeep: |+\n  # text\n\n # Trail\n  # comments.\n",
    "{\n  \"strip\": \"# text\",\n  \"clip\": \"# text\\n\",\n  \"keep\": \"# text\\n\\n\"\n}\n"
);

// FBC9
json_case!(
    yaml_suite_fbc9,
    "safe: a!\"#$%&'()*+,-./09:;<=>?@AZ[\\]^_`az{|}~\n     !\"#$%&'()*+,-./09:;<=>?@AZ[\\]^_`az{|}~\nsafe question mark: ?foo\nsafe colon: :foo\nsafe dash: -foo\n",
    "{\n  \"safe\": \"a!\\\"#$%&'()*+,-./09:;<=>?@AZ[\\\\]^_`az{|}~ !\\\"#$%&'()*+,-./09:;<=>?@AZ[\\\\]^_`az{|}~\",\n  \"safe question mark\": \"?foo\",\n  \"safe colon\": \":foo\",\n  \"safe dash\": \"-foo\"\n}\n"
);

// FQ7F
json_case!(
    yaml_suite_fq7f,
    "- Mark McGwire\n- Sammy Sosa\n- Ken Griffey\n",
    "[\n  \"Mark McGwire\",\n  \"Sammy Sosa\",\n  \"Ken Griffey\"\n]\n"
);

// FTA2
json_case!(yaml_suite_fta2, "--- &sequence\n- a\n", "[\n  \"a\"\n]\n");

// FUP4
json_case!(
    yaml_suite_fup4,
    "[a, [b, c]]\n",
    "[\n  \"a\",\n  [\n    \"b\",\n    \"c\"\n  ]\n]\n"
);

// G4RS
json_case!(
    yaml_suite_g4rs,
    "unicode: \"Sosa did fine.\\u263A\"\ncontrol: \"\\b1998\\t1999\\t2000\\n\"\nhex esc: \"\\x0d\\x0a is \\r\\n\"\n\nsingle: '\"Howdy!\" he cried.'\nquoted: ' # Not a ''comment''.'\ntie-fighter: '|\\-*-/|'\n",
    "{\n  \"unicode\": \"Sosa did fine.\u{263a}\",\n  \"control\": \"\\b1998\\t1999\\t2000\\n\",\n  \"hex esc\": \"\\r\\n is \\r\\n\",\n  \"single\": \"\\\"Howdy!\\\" he cried.\",\n  \"quoted\": \" # Not a 'comment'.\",\n  \"tie-fighter\": \"|\\\\-*-/|\"\n}\n"
);

// G992
json_case!(
    yaml_suite_g992,
    ">\n folded\n text\n\n\n",
    "\"folded text\\n\"\n"
);

// GH63
json_case!(
    yaml_suite_gh63,
    "? a\n: 1.3\nfifteen: d\n",
    "{\n  \"a\": 1.3,\n  \"fifteen\": \"d\"\n}\n"
);

// H2RW
json_case!(
    yaml_suite_h2rw,
    "foo: 1\n\nbar: 2\n    \ntext: |\n  a\n    \n  b\n\n  c\n \n  d\n",
    "{\n  \"foo\": 1,\n  \"bar\": 2,\n  \"text\": \"a\\n  \\nb\\n\\nc\\n\\nd\\n\"\n}\n"
);

// H3Z8
json_case!(
    yaml_suite_h3z8,
    "---\nwanted: love \u{2665} and peace \u{262e}\n",
    "{\n  \"wanted\": \"love \u{2665} and peace \u{262e}\"\n}\n"
);

// HM87/00
json_case!(yaml_suite_hm87_00, "[:x]\n", "[\n  \":x\"\n]\n");

// HM87/01
json_case!(yaml_suite_hm87_01, "[?x]\n", "[\n  \"?x\"\n]\n");

// HMK4
json_case!(
    yaml_suite_hmk4,
    "name: Mark McGwire\naccomplishment: >\n  Mark set a major league\n  home run record in 1998.\nstats: |\n  65 Home Runs\n  0.278 Batting Average\n",
    "{\n  \"name\": \"Mark McGwire\",\n  \"accomplishment\": \"Mark set a major league home run record in 1998.\\n\",\n  \"stats\": \"65 Home Runs\\n0.278 Batting Average\\n\"\n}\n"
);

// HMQ5
json_case!(
    yaml_suite_hmq5,
    "!!str &a1 \"foo\":\n  !!str bar\n&a2 baz : *a1\n",
    "{\n  \"foo\": \"bar\",\n  \"baz\": \"foo\"\n}\n"
);

// HS5T
json_case!(
    yaml_suite_hs5t,
    "1st non-empty\n\n 2nd non-empty \n\t3rd non-empty\n",
    "\"1st non-empty\\n2nd non-empty 3rd non-empty\"\n"
);

// HWV9
json_case!(yaml_suite_hwv9, "...\n", "");

// JS2J
json_case!(
    yaml_suite_js2j,
    "First occurrence: &anchor Value\nSecond occurrence: *anchor\n",
    "{\n  \"First occurrence\": \"Value\",\n  \"Second occurrence\": \"Value\"\n}\n"
);

// JTV5
json_case!(
    yaml_suite_jtv5,
    "? a\n  true\n: null\n  d\n? e\n  42\n",
    "{\n  \"a true\": \"null d\",\n  \"e 42\": null\n}\n"
);

// K3WX
json_case!(
    yaml_suite_k3wx,
    "---\n{ \"foo\" # comment\n  :bar }\n",
    "{\n  \"foo\": \"bar\"\n}\n"
);

// K4SU
json_case!(
    yaml_suite_k4su,
    "- foo\n- bar\n- 42\n",
    "[\n  \"foo\",\n  \"bar\",\n  42\n]\n"
);

// K527
json_case!(
    yaml_suite_k527,
    ">-\n  trimmed\n  \n \n\n  as\n  space\n",
    "\"trimmed\\n\\n\\nas space\"\n"
);

// K54U
json_case!(yaml_suite_k54u, "---\tscalar\n", "\"scalar\"\n");

// K858
json_case!(
    yaml_suite_k858,
    "strip: >-\n\nclip: >\n\nkeep: |+\n\n",
    "{\n  \"strip\": \"\",\n  \"clip\": \"\",\n  \"keep\": \"\\n\"\n}\n"
);

// KH5V/00
json_case!(
    yaml_suite_kh5v_00,
    "\"1 inline\\ttab\"\n",
    "\"1 inline\\ttab\"\n"
);

// KH5V/01
json_case!(
    yaml_suite_kh5v_01,
    "\"2 inline\\\ttab\"\n",
    "\"2 inline\\ttab\"\n"
);

// KH5V/02
json_case!(
    yaml_suite_kh5v_02,
    "\"3 inline\ttab\"\n",
    "\"3 inline\\ttab\"\n"
);

// KMK3
json_case!(
    yaml_suite_kmk3,
    "foo:\n  bar: 1\nbaz: 2\n",
    "{\n  \"foo\": {\n    \"bar\": 1\n  },\n  \"baz\": 2\n}\n"
);

// KSS4
json_case!(
    yaml_suite_kss4,
    "--- \"quoted\nstring\"\n--- &node foo\n",
    "\"quoted string\"\n\"foo\"\n"
);

// L24T/00
json_case!(
    yaml_suite_l24t_00,
    "foo: |\n  x\n   \n",
    "{\n  \"foo\" : \"x\\n \\n\"\n}\n"
);

// L24T/01
json_case!(
    yaml_suite_l24t_01,
    "foo: |\n  x\n   ",
    "{\n  \"foo\" : \"x\\n \\n\"\n}\n"
);

// L9U5
json_case!(
    yaml_suite_l9u5,
    "implicit block key : [\n  implicit flow key : value,\n ]\n",
    "{\n  \"implicit block key\": [\n    {\n      \"implicit flow key\": \"value\"\n    }\n  ]\n}\n"
);
